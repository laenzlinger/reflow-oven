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
    pub soak_target: f32,      // °C
    pub soak_duration_s: u32,  // seconds at soak temp
    pub reflow_target: f32,    // °C (peak)
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
            soak_target: 170.0,
            soak_duration_s: 60,
            reflow_target: 210.0,
            reflow_duration_s: 30,
            cooling_target: 50.0,
        }
    }

    /// Low-temp Sn42/Bi58 (melting point 138°C)
    pub fn sn42_bi58() -> Self {
        Self {
            preheat_target: 100.0,
            soak_target: 130.0,
            soak_duration_s: 60,
            reflow_target: 165.0,
            reflow_duration_s: 30,
            cooling_target: 50.0,
        }
    }
}

pub struct ProfileRunner {
    pub profile: Profile,
    pub phase: Phase,
    phase_elapsed_s: f32,
}

impl ProfileRunner {
    pub fn new(profile: Profile) -> Self {
        Self { profile, phase: Phase::Idle, phase_elapsed_s: 0.0 }
    }

    pub fn start(&mut self) {
        self.phase = Phase::Preheat;
        self.phase_elapsed_s = 0.0;
    }

    pub fn stop(&mut self) {
        self.phase = Phase::Idle;
        self.phase_elapsed_s = 0.0;
    }

    /// Returns the target temperature for the current phase.
    pub fn target_temperature(&self) -> f32 {
        match self.phase {
            Phase::Idle | Phase::Done => 0.0,
            Phase::Preheat => self.profile.preheat_target,
            Phase::Soak => self.profile.soak_target,
            Phase::Reflow => self.profile.reflow_target,
            Phase::Cooling => self.profile.cooling_target,
        }
    }

    /// Advance state machine given current temperature and dt.
    pub fn update(&mut self, temp: f32, dt: f32) {
        self.phase_elapsed_s += dt;
        match self.phase {
            Phase::Preheat if temp >= self.profile.preheat_target => {
                self.phase = Phase::Soak;
                self.phase_elapsed_s = 0.0;
            }
            Phase::Soak if self.phase_elapsed_s >= self.profile.soak_duration_s as f32 => {
                self.phase = Phase::Reflow;
                self.phase_elapsed_s = 0.0;
            }
            Phase::Reflow if self.phase_elapsed_s >= self.profile.reflow_duration_s as f32 => {
                self.phase = Phase::Cooling;
                self.phase_elapsed_s = 0.0;
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
    fn target_temperature_matches_phase() {
        let mut runner = ProfileRunner::new(Profile::sn63_pb37());
        assert_eq!(runner.target_temperature(), 0.0);
        runner.start();
        assert_eq!(runner.target_temperature(), 150.0);
    }
}
