use std::f32::consts::{PI, TAU};

use crate::math::{Affine, Vec3};

pub const DEFAULT_FOV: f32 = 50.0;
pub const DEFAULT_ZOOM: f32 = 0.76;

const MIN_DISTANCE: f32 = 10.0;
const MAX_DISTANCE: f32 = 256.0;
const POLAR_EPSILON: f32 = 1e-6;
const WHEEL_SCALE: f32 = 0.95;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrbitCamera {
    theta: f32,
    phi: f32,
    distance: f32,
    fov: f32,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        Self::new(DEFAULT_FOV, DEFAULT_ZOOM)
    }
}

impl OrbitCamera {
    pub fn new(fov_degrees: f32, zoom: f32) -> Self {
        let half_fov = (fov_degrees / 2.0).to_radians();
        Self {
            theta: 0.0,
            phi: PI / 2.0,
            distance: (4.5 + 16.5 / half_fov.tan() / zoom).clamp(MIN_DISTANCE, MAX_DISTANCE),
            fov: fov_degrees,
        }
    }

    pub fn fov(&self) -> f32 {
        self.fov
    }

    pub fn distance(&self) -> f32 {
        self.distance
    }

    pub fn drag(&mut self, dx: f32, dy: f32, viewport_height: f32) {
        if viewport_height <= 0.0 {
            return;
        }
        self.theta -= TAU * dx / viewport_height;
        self.phi = (self.phi - TAU * dy / viewport_height).clamp(POLAR_EPSILON, PI - POLAR_EPSILON);
    }

    pub fn wheel(&mut self, delta_y: f32) {
        let factor = if delta_y < 0.0 {
            WHEEL_SCALE
        } else if delta_y > 0.0 {
            1.0 / WHEEL_SCALE
        } else {
            return;
        };
        self.distance = (self.distance * factor).clamp(MIN_DISTANCE, MAX_DISTANCE);
    }

    pub(crate) fn position(&self) -> Vec3 {
        let (sin_phi, cos_phi) = self.phi.sin_cos();
        let (sin_theta, cos_theta) = self.theta.sin_cos();
        Vec3::new(sin_phi * sin_theta, cos_phi, sin_phi * cos_theta) * self.distance
    }

    pub(crate) fn view(&self) -> Affine {
        let eye = self.position();
        let forward_back = eye.normalized();
        let side = Vec3::new(0.0, 1.0, 0.0).cross(forward_back);
        let side = if side.length() > 1e-12 {
            side.normalized()
        } else {
            Vec3::new(1.0, 0.0, 0.0)
        };
        let up = forward_back.cross(side);
        Affine::from_basis(side, up, forward_back, eye).inverse_rigid()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_distance_matches_skinview3d() {
        let camera = OrbitCamera::default();
        assert!((camera.distance() - 51.058_374).abs() < 1e-3);
        assert!((camera.position().z - camera.distance()).abs() < 1e-3);
    }

    #[test]
    fn view_puts_origin_in_front() {
        let mut camera = OrbitCamera::default();
        camera.drag(37.0, -12.0, 320.0);
        let origin = camera.view().point(Vec3::default());
        assert!(origin.x.abs() < 1e-3 && origin.y.abs() < 1e-3);
        assert!((origin.z + camera.distance()).abs() < 1e-3);
    }

    #[test]
    fn drag_and_wheel_follow_orbit_controls() {
        let mut camera = OrbitCamera::default();
        camera.drag(80.0, 0.0, 320.0);
        assert!((camera.theta + TAU * 0.25).abs() < 1e-6);
        camera.drag(0.0, 10_000.0, 320.0);
        assert_eq!(camera.phi, POLAR_EPSILON);
        let before = camera.distance();
        camera.wheel(-120.0);
        assert!((camera.distance() - before * WHEEL_SCALE).abs() < 1e-4);
        camera.wheel(0.0);
        for _ in 0..200 {
            camera.wheel(-1.0);
        }
        assert_eq!(camera.distance(), MIN_DISTANCE);
        for _ in 0..200 {
            camera.wheel(1.0);
        }
        assert_eq!(camera.distance(), MAX_DISTANCE);
    }
}
