use esp_idf_svc::hal::gpio::{Output, PinDriver};
use std::time::Instant;

/// Slow PWM driver for SSR (period ~1-2s, suitable for thermal loads).
pub struct Ssr<'a> {
    pin: PinDriver<'a, Output>,
    period_ms: u32,
    duty_pct: f32,
    cycle_start: Instant,
}

impl<'a> Ssr<'a> {
    pub fn new(pin: PinDriver<'a, Output>, period_ms: u32) -> Self {
        Self { pin, period_ms, duty_pct: 0.0, cycle_start: Instant::now() }
    }

    pub fn set_duty(&mut self, pct: f32) {
        self.duty_pct = pct.clamp(0.0, 100.0);
    }

    /// Call this in the control loop to update SSR on/off state.
    pub fn tick(&mut self) {
        let elapsed = self.cycle_start.elapsed().as_millis() as u32;
        if elapsed >= self.period_ms {
            self.cycle_start = Instant::now();
        }
        let on_time = (self.duty_pct / 100.0 * self.period_ms as f32) as u32;
        let elapsed = self.cycle_start.elapsed().as_millis() as u32;
        if elapsed < on_time {
            let _ = self.pin.set_high();
        } else {
            let _ = self.pin.set_low();
        }
    }
}
