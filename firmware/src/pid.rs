pub struct Pid {
    kp: f32,
    ki: f32,
    kd: f32,
    setpoint: f32,
    integral: f32,
    prev_error: f32,
    output_min: f32,
    output_max: f32,
}

impl Pid {
    pub fn new(kp: f32, ki: f32, kd: f32) -> Self {
        Self {
            kp,
            ki,
            kd,
            setpoint: 0.0,
            integral: 0.0,
            prev_error: 0.0,
            output_min: 0.0,
            output_max: 100.0,
        }
    }

    pub fn set_target(&mut self, setpoint: f32) {
        self.setpoint = setpoint;
    }

    pub fn reset(&mut self) {
        self.integral = 0.0;
        self.prev_error = 0.0;
    }

    /// Compute PID output (0..100%) given current temperature and dt in seconds.
    pub fn update(&mut self, measured: f32, dt: f32) -> f32 {
        let error = self.setpoint - measured;
        self.integral += error * dt;
        let derivative = (error - self.prev_error) / dt;
        self.prev_error = error;

        let output = self.kp * error + self.ki * self.integral + self.kd * derivative;
        output.clamp(self.output_min, self.output_max)
    }
}
