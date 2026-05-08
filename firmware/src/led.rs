use anyhow::Result;
use esp_idf_svc::hal::gpio::OutputPin;
use esp_idf_svc::hal::rmt::config::TransmitConfig;
use esp_idf_svc::hal::rmt::{FixedLengthSignal, PinState, Pulse, RmtChannel, TxRmtDriver};
use std::time::Duration;

use crate::profile::Phase;

pub struct StatusLed<'a> {
    tx: TxRmtDriver<'a>,
}

impl<'a> StatusLed<'a> {
    pub fn new(channel: impl RmtChannel + 'a, pin: impl OutputPin + 'a) -> Result<Self> {
        let config = TransmitConfig::new()
            .clock_divider(2)
            .idle(Some(PinState::Low));
        let tx = TxRmtDriver::new(channel, pin, &config)?;
        Ok(Self { tx })
    }

    pub fn set_color(&mut self, r: u8, g: u8, b: u8) -> Result<()> {
        let ticks_hz = self.tx.counter_clock()?;
        let t0h = Pulse::new_with_duration(ticks_hz, PinState::High, &Duration::from_nanos(350))?;
        let t0l = Pulse::new_with_duration(ticks_hz, PinState::Low, &Duration::from_nanos(800))?;
        let t1h = Pulse::new_with_duration(ticks_hz, PinState::High, &Duration::from_nanos(700))?;
        let t1l = Pulse::new_with_duration(ticks_hz, PinState::Low, &Duration::from_nanos(600))?;

        // WS2812: GRB, MSB first
        let color: u32 = ((g as u32) << 16) | ((r as u32) << 8) | b as u32;
        let mut signal = FixedLengthSignal::<24>::new();
        for i in (0..24).rev() {
            let bit = (color >> i) & 1 != 0;
            let (high, low) = if bit { (t1h, t1l) } else { (t0h, t0l) };
            signal.set(23 - i as usize, &(high, low))?;
        }
        self.tx.start_blocking(&signal)?;
        Ok(())
    }

    pub fn update(&mut self, phase: Phase) {
        let (r, g, b) = match phase {
            Phase::Idle => (0, 0, 25),
            Phase::Preheat => (25, 12, 0),
            Phase::Soak => (25, 25, 0),
            Phase::Reflow => (25, 0, 0),
            Phase::Cooling => (0, 12, 25),
            Phase::Done => (0, 25, 0),
        };
        let _ = self.set_color(r, g, b);
    }
}
