#[cfg(not(test))]
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(not(test), derive(Serialize, Deserialize))]
pub enum Phase {
    Idle,
    Preheat,
    Soak,
    Reflow,
    Cooling,
    Done,
}

#[derive(Debug, Clone)]
#[cfg_attr(not(test), derive(Serialize, Deserialize))]
pub struct Profile {
    pub preheat_target: f32,   // °C
    pub preheat_ramp: f32,     // °C/s ramp rate to preheat
    pub soak_target: f32,      // °C
    pub soak_ramp: f32,        // °C/s ramp rate to soak
    pub soak_duration_s: u32,  // seconds at soak temp
    pub reflow_target: f32,    // °C (peak)
    pub reflow_ramp: f32,      // °C/s ramp rate to reflow
    pub reflow_duration_s: u32,
    pub cooling_target: f32,   // °C to reach before done
}

impl Default for Profile {
    fn default() -> Self {
        Self::sn63_pb37()
    }
}

impl Profile {
    /// Leaded Sn63/Pb37 (Relife HW21, melting point 183°C)
    pub fn sn63_pb37() -> Self {
        Self {
            preheat_target: 150.0,
            preheat_ramp: 2.0,    // °C/s
            soak_target: 170.0,
            soak_ramp: 0.5,       // °C/s (gentle ramp during soak)
            soak_duration_s: 60,
            reflow_target: 220.0,
            reflow_ramp: 1.5,     // °C/s
            reflow_duration_s: 45,
            cooling_target: 50.0,
        }
    }

    /// Low-temp Sn42/Bi58 (melting point 138°C)
    pub fn sn42_bi58() -> Self {
        Self {
            preheat_target: 100.0,
            preheat_ramp: 2.0,
            soak_target: 130.0,
            soak_ramp: 0.5,
            soak_duration_s: 60,
            reflow_target: 165.0,
            reflow_ramp: 2.0,
            reflow_duration_s: 30,
            cooling_target: 50.0,
        }
    }
}

pub struct ProfileRunner {
    pub profile: Profile,
    pub phase: Phase,
    phase_elapsed_s: f32,
    ramped_target: f32,
}

impl ProfileRunner {
    pub fn new(profile: Profile) -> Self {
        Self { profile, phase: Phase::Idle, phase_elapsed_s: 0.0, ramped_target: 0.0 }
    }

    pub fn start(&mut self) {
        self.phase = Phase::Preheat;
        self.phase_elapsed_s = 0.0;
        self.ramped_target = 0.0;
    }

    pub fn stop(&mut self) {
        self.phase = Phase::Idle;
        self.phase_elapsed_s = 0.0;
        self.ramped_target = 0.0;
    }

    /// Returns the ramped target temperature for the current phase.
    pub fn target_temperature(&self) -> f32 {
        match self.phase {
            Phase::Idle | Phase::Done | Phase::Cooling => 0.0,
            _ => self.ramped_target,
        }
    }

    /// Advance state machine given current temperature and dt.
    pub fn update(&mut self, temp: f32, dt: f32) {
        self.phase_elapsed_s += dt;
        match self.phase {
            Phase::Preheat => {
                self.ramped_target = (self.ramped_target + self.profile.preheat_ramp * dt)
                    .min(self.profile.preheat_target);
                if temp >= self.profile.preheat_target {
                    self.phase = Phase::Soak;
                    self.phase_elapsed_s = 0.0;
                    self.ramped_target = temp;
                }
            }
            Phase::Soak => {
                self.ramped_target = (self.ramped_target + self.profile.soak_ramp * dt)
                    .min(self.profile.soak_target);
                if self.phase_elapsed_s >= self.profile.soak_duration_s as f32 {
                    self.phase = Phase::Reflow;
                    self.phase_elapsed_s = 0.0;
                    self.ramped_target = temp;
                }
            }
            Phase::Reflow => {
                self.ramped_target = (self.ramped_target + self.profile.reflow_ramp * dt)
                    .min(self.profile.reflow_target);
                if self.phase_elapsed_s >= self.profile.reflow_duration_s as f32 {
                    self.phase = Phase::Cooling;
                    self.phase_elapsed_s = 0.0;
                }
            }
            Phase::Cooling if temp <= self.profile.cooling_target => {
                self.phase = Phase::Done;
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_idle() {
        let runner = ProfileRunner::new(Profile::default());
        assert_eq!(runner.phase, Phase::Idle);
    }

    #[test]
    fn start_enters_preheat() {
        let mut runner = ProfileRunner::new(Profile::default());
        runner.start();
        assert_eq!(runner.phase, Phase::Preheat);
    }

    #[test]
    fn preheat_to_soak_on_target() {
        let mut runner = ProfileRunner::new(Profile::sn63_pb37());
        runner.start();
        runner.update(150.0, 0.25);
        assert_eq!(runner.phase, Phase::Soak);
    }

    #[test]
    fn ramped_target_increases_gradually() {
        let mut runner = ProfileRunner::new(Profile::sn63_pb37());
        runner.start();
        runner.update(25.0, 0.25); // ramp = 2°C/s, dt=0.25 → +0.5°C
        assert!((runner.target_temperature() - 0.5).abs() < 0.01);
        runner.update(25.0, 0.25);
        assert!((runner.target_temperature() - 1.0).abs() < 0.01);
    }

    #[test]
    fn ramped_target_capped_at_phase_target() {
        let mut runner = ProfileRunner::new(Profile::sn63_pb37());
        runner.start();
        // Advance 100s at 2°C/s = 200°C ramp, but capped at 150°C
        for _ in 0..400 {
            runner.update(25.0, 0.25);
        }
        assert!((runner.target_temperature() - 150.0).abs() < 0.01);
    }

    #[test]
    fn soak_to_reflow_after_duration() {
        let mut runner = ProfileRunner::new(Profile::sn63_pb37());
        runner.start();
        runner.update(150.0, 0.25); // → Soak
        // Advance 60 seconds
        for _ in 0..240 {
            runner.update(170.0, 0.25);
        }
        assert_eq!(runner.phase, Phase::Reflow);
    }

    #[test]
    fn reflow_to_cooling_after_duration() {
        let mut runner = ProfileRunner::new(Profile::sn63_pb37());
        runner.start();
        runner.update(150.0, 0.25); // → Soak
        for _ in 0..240 { runner.update(170.0, 0.25); } // → Reflow
        for _ in 0..120 { runner.update(210.0, 0.25); } // 30s → Cooling
        assert_eq!(runner.phase, Phase::Cooling);
    }

    #[test]
    fn cooling_to_done_on_target() {
        let mut runner = ProfileRunner::new(Profile::sn63_pb37());
        runner.start();
        runner.update(150.0, 0.25);
        for _ in 0..240 { runner.update(170.0, 0.25); }
        for _ in 0..120 { runner.update(210.0, 0.25); }
        runner.update(50.0, 0.25);
        assert_eq!(runner.phase, Phase::Done);
    }

    #[test]
    fn stop_returns_to_idle() {
        let mut runner = ProfileRunner::new(Profile::default());
        runner.start();
        runner.stop();
        assert_eq!(runner.phase, Phase::Idle);
    }

    #[test]
    fn target_temperature_zero_when_idle() {
        let runner = ProfileRunner::new(Profile::sn63_pb37());
        assert_eq!(runner.target_temperature(), 0.0);
    }
}
