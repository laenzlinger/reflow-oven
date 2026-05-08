use anyhow::Result;
use esp_idf_svc::hal::adc::oneshot::{AdcChannelDriver, AdcDriver};
use esp_idf_svc::hal::adc::AdcChannel;
use std::borrow::Borrow;

pub trait TemperatureSensor {
    fn read_celsius(&mut self) -> Result<f32>;
}

/// NTC 100K B3950 thermistor via voltage divider on ADC.
/// Wiring: 3.3V --- R_series --- ADC_pin --- NTC --- GND
pub struct NtcThermistor<'a, C, M>
where
    C: AdcChannel,
    M: Borrow<AdcDriver<'a, C::AdcUnit>>,
{
    channel: AdcChannelDriver<'a, C, M>,
    r_series: f32,
    b_coefficient: f32,
    r_nominal: f32,
    t_nominal: f32,
}

impl<'a, C, M> NtcThermistor<'a, C, M>
where
    C: AdcChannel,
    M: Borrow<AdcDriver<'a, C::AdcUnit>>,
{
    pub fn new(channel: AdcChannelDriver<'a, C, M>) -> Self {
        Self {
            channel,
            r_series: 100_000.0,
            b_coefficient: 3950.0,
            r_nominal: 100_000.0,
            t_nominal: 25.0,
        }
    }
}

impl<'a, C, M> TemperatureSensor for NtcThermistor<'a, C, M>
where
    C: AdcChannel,
    M: Borrow<AdcDriver<'a, C::AdcUnit>>,
{
    fn read_celsius(&mut self) -> Result<f32> {
        let raw: u16 = self.channel.read_raw()?;
        let max_adc = 4095.0_f32;
        let r_ntc = self.r_series * (max_adc / raw as f32 - 1.0);

        // B-parameter Steinhart-Hart
        let inv_t = 1.0 / (self.t_nominal + 273.15)
            + (1.0 / self.b_coefficient) * (r_ntc / self.r_nominal).ln();
        let temp_c = 1.0 / inv_t - 273.15;

        Ok(temp_c)
    }
}

/// Simulated oven sensor for testing without hardware.
/// Models a simple thermal system: heats when duty > 0, cools toward ambient.
pub struct SimulatedSensor {
    temperature: f32,
    ambient: f32,
    duty_pct: f32,
}

impl SimulatedSensor {
    pub fn new() -> Self {
        Self { temperature: 25.0, ambient: 25.0, duty_pct: 0.0 }
    }

    pub fn set_duty(&mut self, duty: f32) {
        self.duty_pct = duty;
    }

    /// Advance simulation by dt seconds.
    pub fn tick(&mut self, dt: f32) {
        // Simple model: max heating rate ~3°C/s at 100% duty, cooling ~0.5°C/s toward ambient
        let heat_rate = self.duty_pct / 100.0 * 3.0;
        let cool_rate = (self.temperature - self.ambient) * 0.005;
        self.temperature += (heat_rate - cool_rate) * dt;
    }
}

impl TemperatureSensor for SimulatedSensor {
    fn read_celsius(&mut self) -> Result<f32> {
        Ok(self.temperature)
    }
}
