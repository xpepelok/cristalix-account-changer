use std::f32::consts::PI;

const REST_CAPE_PITCH: f32 = 10.8 * PI / 180.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pose {
    pub left_arm_roll: f32,
    pub right_arm_roll: f32,
    pub cape_pitch: f32,
}

impl Default for Pose {
    fn default() -> Self {
        Self {
            left_arm_roll: 0.0,
            right_arm_roll: 0.0,
            cape_pitch: REST_CAPE_PITCH,
        }
    }
}

impl Pose {
    pub fn idle(seconds: f32) -> Self {
        let t = seconds * 2.0;
        let spread = 0.02 * PI;
        Self {
            left_arm_roll: 0.03 * t.cos() + spread,
            right_arm_roll: 0.03 * (t + PI).cos() - spread,
            cape_pitch: 0.01 * t.sin() + 0.06 * PI,
        }
    }
}
