use anyhow::Result;
use esp_idf_svc::hal::gpio::{Input, Output, PinDriver};

pub trait TemperatureSensor {
    fn read_celsius(&mut self) -> Result<f32>;
}

/// MAX31855 thermocouple-to-digital converter via bit-banged SPI.
/// Reads 14-bit thermocouple temperature with 0.25°C resolution.
pub struct Max31855<'a> {
    cs: PinDriver<'a, Output>,
    sck: PinDriver<'a, Output>,
    so: PinDriver<'a, Input>,
}

impl<'a> Max31855<'a> {
    pub fn new(
        mut cs: PinDriver<'a, Output>,
        mut sck: PinDriver<'a, Output>,
        so: PinDriver<'a, Input>,
    ) -> Self {
        let _ = cs.set_high();
        let _ = sck.set_low();
        Self { cs, sck, so }
    }

    fn read_raw(&mut self) -> u32 {
        let _ = self.cs.set_low();
        std::thread::sleep(std::time::Duration::from_micros(1));

        let mut data: u32 = 0;
        for _ in 0..32 {
            let _ = self.sck.set_high();
            std::thread::sleep(std::time::Duration::from_micros(1));
            data <<= 1;
            if self.so.is_high() {
                data |= 1;
            }
            let _ = self.sck.set_low();
            std::thread::sleep(std::time::Duration::from_micros(1));
        }

        let _ = self.cs.set_high();
        data
    }
}

impl<'a> TemperatureSensor for Max31855<'a> {
    fn read_celsius(&mut self) -> Result<f32> {
        let raw = self.read_raw();

        // Bit 16 = fault flag
        if raw & 0x10000 != 0 {
            anyhow::bail!("MAX31855 fault: {:#x}", raw & 0x07);
        }

        // Thermocouple temp: bits 31..18 (14-bit signed, 0.25°C resolution)
        let tc_raw = (raw >> 18) as i16;
        // Sign-extend from 14 bits
        let tc_signed = if tc_raw & 0x2000 != 0 {
            (tc_raw | !0x3FFF) as f32
        } else {
            tc_raw as f32
        };

        Ok(tc_signed * 0.25)
    }
}

/// Simulated oven sensor for testing without hardware.
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

    pub fn tick(&mut self, dt: f32) {
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
