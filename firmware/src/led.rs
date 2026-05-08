use anyhow::Result;
use esp_idf_svc::hal::gpio::OutputPin;
use esp_idf_svc::hal::rmt::config::{TxChannelConfig, TransmitConfig};
use esp_idf_svc::hal::rmt::encoder::BytesEncoderConfig;
use esp_idf_svc::hal::rmt::{PinState, Pulse, PulseTicks, Symbol, TxChannelDriver};
use esp_idf_svc::hal::units::FromValueType;

use crate::profile::Phase;

pub struct StatusLed<'a> {
    tx: TxChannelDriver<'a>,
    encoder_config: BytesEncoderConfig,
}

impl<'a> StatusLed<'a> {
    pub fn new(pin: impl OutputPin + 'a) -> Result<Self> {
        // 10MHz resolution = 100ns per tick
        let channel_config = TxChannelConfig {
            resolution: 10.MHz().into(),
            ..Default::default()
        };
        let tx = TxChannelDriver::new(pin, &channel_config)?;

        // WS2812 timings at 10MHz (100ns/tick):
        // T0H=400ns=4t, T0L=850ns=9t, T1H=800ns=8t, T1L=450ns=5t
        let bit0 = Symbol::new(
            Pulse::new(PinState::High, PulseTicks::new(4).unwrap()),
            Pulse::new(PinState::Low, PulseTicks::new(9).unwrap()),
        );
        let bit1 = Symbol::new(
            Pulse::new(PinState::High, PulseTicks::new(8).unwrap()),
            Pulse::new(PinState::Low, PulseTicks::new(5).unwrap()),
        );
        let encoder_config = BytesEncoderConfig {
            bit0,
            bit1,
            msb_first: true,
            ..Default::default()
        };

        Ok(Self { tx, encoder_config })
    }

    pub fn set_color(&mut self, r: u8, g: u8, b: u8) -> Result<()> {
        use esp_idf_svc::hal::rmt::encoder::BytesEncoder;
        let encoder = BytesEncoder::with_config(&self.encoder_config)?;
        let data = [g, r, b]; // WS2812: GRB order
        let tx_config = TransmitConfig::default();
        self.tx.send_and_wait(encoder, &data, &tx_config)?;
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
