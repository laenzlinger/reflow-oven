mod led;
mod pid;
mod profile;
mod sensor;
mod ssr;
mod web;

use anyhow::Result;
use esp_idf_svc::hal::adc::attenuation;
use esp_idf_svc::hal::adc::oneshot::config::AdcChannelConfig;
use esp_idf_svc::hal::adc::oneshot::{AdcChannelDriver, AdcDriver};
use esp_idf_svc::hal::gpio::PinDriver;
use esp_idf_svc::hal::peripherals::Peripherals;
use esp_idf_svc::http::Method;
use esp_idf_svc::wifi::{BlockingWifi, ClientConfiguration, Configuration, EspWifi};
use esp_idf_svc::{eventloop::EspSystemEventLoop, nvs::EspDefaultNvsPartition};
use log::info;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use pid::Pid;
use profile::{Phase, Profile, ProfileRunner};
use sensor::{NtcThermistor, SimulatedSensor, TemperatureSensor};
use ssr::Ssr;
use web::{History, OvenState, SharedHistory, SharedState};

const WIFI_SSID: &str = env!("WIFI_SSID");
const WIFI_PASS: &str = env!("WIFI_PASS");

fn main() -> Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take()?;
    let sysloop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;

    // Status LED (NeoPixel on GPIO48) - purple = connecting
    let mut status_led = led::StatusLed::new(peripherals.pins.gpio48)?;
    let _ = status_led.set_color(15, 0, 15); // purple = connecting WiFi

    // WiFi
    let mut wifi = BlockingWifi::wrap(
        EspWifi::new(peripherals.modem, sysloop.clone(), Some(nvs))?,
        sysloop,
    )?;
    wifi.set_configuration(&Configuration::Client(ClientConfiguration {
        ssid: WIFI_SSID.try_into().unwrap(),
        password: WIFI_PASS.try_into().unwrap(),
        ..Default::default()
    }))?;
    // Set hostname before DHCP so router sees it
    {
        use esp_idf_svc::handle::RawHandle;
        let hostname = std::ffi::CString::new("reflow-oven").unwrap();
        unsafe {
            esp_idf_svc::sys::esp_netif_set_hostname(
                wifi.wifi().sta_netif().handle(),
                hostname.as_ptr(),
            );
        }
    }
    wifi.start()?;
    wifi.connect()?;
    wifi.wait_netif_up()?;
    unsafe { esp_idf_svc::sys::esp_wifi_set_ps(esp_idf_svc::sys::wifi_ps_type_t_WIFI_PS_NONE); }
    let ip = wifi.wifi().sta_netif().get_ip_info()?.ip;
    info!("WiFi connected, IP: {} (hostname: reflow-oven)", ip);
    status_led.update(Phase::Idle); // blue = connected

    // Shared state
    let state: SharedState = Arc::new(Mutex::new(OvenState::default()));
    let history: SharedHistory = Arc::new(Mutex::new(History::new()));
    let cmd: Arc<Mutex<Option<Cmd>>> = Arc::new(Mutex::new(None));

    // Web server
    let mut server = web::start_server(state.clone(), history.clone())?;
    let cmd_start = cmd.clone();
    server.fn_handler("/start", Method::Post, move |req| {
        *cmd_start.lock().unwrap() = Some(Cmd::Start);
        req.into_ok_response().map(|_| ())
    })?;
    let cmd_stop = cmd.clone();
    server.fn_handler("/stop", Method::Post, move |req| {
        *cmd_stop.lock().unwrap() = Some(Cmd::Stop);
        req.into_ok_response().map(|_| ())
    })?;
    let cmd_sim = cmd.clone();
    server.fn_handler("/simulate", Method::Post, move |req| {
        *cmd_sim.lock().unwrap() = Some(Cmd::ToggleSimulate);
        req.into_ok_response().map(|_| ())
    })?;
    let cmd_profile = cmd.clone();
    server.fn_handler("/profile", Method::Post, move |mut req| {
        let mut buf = [0u8; 32];
        let len = req.read(&mut buf).unwrap_or(0);
        let name = std::str::from_utf8(&buf[..len]).unwrap_or("");
        let profile_cmd = match name {
            "sn42bi58" => Some(Cmd::SetProfile(Profile::sn42_bi58())),
            _ => Some(Cmd::SetProfile(Profile::sn63_pb37())),
        };
        *cmd_profile.lock().unwrap() = profile_cmd;
        req.into_ok_response().map(|_| ())
    })?;

    // Sensor (NTC on ADC1, GPIO4)
    let adc1 = AdcDriver::new(peripherals.adc1)?;
    let adc_config = AdcChannelConfig {
        attenuation: attenuation::DB_12,
        ..Default::default()
    };
    let adc_channel = AdcChannelDriver::new(&adc1, peripherals.pins.gpio4, &adc_config)?;
    let mut real_sensor = NtcThermistor::new(adc_channel);
    let mut sim_sensor = SimulatedSensor::new();
    let mut simulating = false;

    // SSR on GPIO5
    let ssr_pin = PinDriver::output(peripherals.pins.gpio5)?;
    let mut ssr = Ssr::new(ssr_pin, 2000);

    // PID controller
    let mut pid = Pid::new(2.0, 0.01, 5.0);

    // Profile
    let mut runner = ProfileRunner::new(Profile::default());

    // Control loop (~4 Hz)
    let dt = 0.25_f32;
    let mut elapsed_s: f32 = 0.0;
    let mut last_phase = Phase::Idle;
    loop {
        let now = Instant::now();

        // Handle commands
        if let Some(c) = cmd.lock().unwrap().take() {
            match c {
                Cmd::Start => {
                    runner.start();
                    pid.reset();
                    sim_sensor = SimulatedSensor::new();
                    elapsed_s = 0.0;
                    history.lock().unwrap().clear();
                }
                Cmd::Stop => {
                    runner.stop();
                    pid.reset();
                    ssr.set_duty(0.0);
                }
                Cmd::ToggleSimulate => {
                    simulating = !simulating;
                }
                Cmd::SetProfile(p) => {
                    runner = ProfileRunner::new(p);
                }
            }
        }

        // Read temperature
        let temp = if simulating {
            sim_sensor.tick(dt);
            sim_sensor.read_celsius().unwrap_or(0.0)
        } else {
            real_sensor.read_celsius().unwrap_or(0.0)
        };

        // Update profile state machine
        runner.update(temp, dt);

        // PID
        let mut duty = if runner.phase == Phase::Idle || runner.phase == Phase::Done {
            0.0
        } else {
            pid.set_target(runner.target_temperature());
            pid.update(temp, dt)
        };

        // Over-temperature watchdog: kill heat if >250°C
        const MAX_SAFE_TEMP: f32 = 250.0;
        if temp > MAX_SAFE_TEMP {
            duty = 0.0;
            runner.stop();
            pid.reset();
            log::error!("OVER-TEMPERATURE {:.0}°C > {:.0}°C — heater OFF", temp, MAX_SAFE_TEMP);
        }

        if simulating {
            sim_sensor.set_duty(duty);
        }
        ssr.set_duty(duty);
        ssr.tick();

        // Track elapsed time during active profile
        if runner.phase != Phase::Idle && runner.phase != Phase::Done {
            elapsed_s += dt;
        }

        // Record history during active profile (once per second)
        if runner.phase != Phase::Idle {
            let last_t = history.lock().unwrap().points.last().map(|p| p.t).unwrap_or(-1.0);
            if elapsed_s - last_t >= 1.0 {
                history.lock().unwrap().push(elapsed_s, temp, runner.target_temperature(), runner.phase);
            }
        }

        // Update shared state for web UI
        {
            let mut s = state.lock().unwrap();
            s.temperature = temp;
            s.target = runner.target_temperature();
            s.duty_pct = duty;
            s.phase = runner.phase;
            s.simulating = simulating;
            s.elapsed_s = elapsed_s;
        }

        // Update LED on phase change
        if runner.phase != last_phase {
            status_led.update(runner.phase);
            last_phase = runner.phase;
        }

        // Sleep remainder of loop period
        let elapsed = now.elapsed();
        let period = Duration::from_millis(250);
        if elapsed < period {
            thread::sleep(period - elapsed);
        }
    }
}

enum Cmd {
    Start,
    Stop,
    ToggleSimulate,
    SetProfile(Profile),
}
