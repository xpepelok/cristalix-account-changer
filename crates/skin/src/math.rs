use std::ops::{Add, Mul, Sub};

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn dot(self, other: Self) -> f32 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    pub fn cross(self, other: Self) -> Self {
        Self::new(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }

    pub fn length(self) -> f32 {
        self.dot(self).sqrt()
    }

    pub fn normalized(self) -> Self {
        self * (1.0 / self.length())
    }

    pub fn lerp(self, other: Self, t: f32) -> Self {
        self + (other - self) * t
    }
}

impl Add for Vec3 {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self::new(self.x + other.x, self.y + other.y, self.z + other.z)
    }
}

impl Sub for Vec3 {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Self::new(self.x - other.x, self.y - other.y, self.z - other.z)
    }
}

impl Mul<f32> for Vec3 {
    type Output = Self;
    fn mul(self, factor: f32) -> Self {
        Self::new(self.x * factor, self.y * factor, self.z * factor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Affine {
    rows: [[f32; 4]; 3],
}

impl Affine {
    pub fn from_basis(x: Vec3, y: Vec3, z: Vec3, origin: Vec3) -> Self {
        Self {
            rows: [
                [x.x, y.x, z.x, origin.x],
                [x.y, y.y, z.y, origin.y],
                [x.z, y.z, z.z, origin.z],
            ],
        }
    }

    pub fn translation(offset: Vec3) -> Self {
        Self::from_basis(
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            offset,
        )
    }

    pub fn euler_xyz(position: Vec3, x: f32, y: f32, z: f32) -> Self {
        let (sa, a) = x.sin_cos();
        let (sb, b) = y.sin_cos();
        let (sc, c) = z.sin_cos();
        Self {
            rows: [
                [b * c, -b * sc, sb, position.x],
                [
                    a * sc + sa * sb * c,
                    a * c - sa * sb * sc,
                    -sa * b,
                    position.y,
                ],
                [
                    sa * sc - a * sb * c,
                    sa * c + a * sb * sc,
                    a * b,
                    position.z,
                ],
            ],
        }
    }

    pub fn inverse_rigid(self) -> Self {
        let r = &self.rows;
        let row = |i: usize| Vec3::new(r[i][0], r[i][1], r[i][2]);
        let column = |j: usize| Vec3::new(r[0][j], r[1][j], r[2][j]);
        let t = column(3);
        Self::from_basis(
            row(0),
            row(1),
            row(2),
            Vec3::new(-column(0).dot(t), -column(1).dot(t), -column(2).dot(t)),
        )
    }

    pub fn scaled(self, factor: Vec3) -> Self {
        let mut rows = self.rows;
        for row in &mut rows {
            row[0] *= factor.x;
            row[1] *= factor.y;
            row[2] *= factor.z;
        }
        Self { rows }
    }

    pub fn point(&self, p: Vec3) -> Vec3 {
        let r = &self.rows;
        Vec3::new(
            r[0][0] * p.x + r[0][1] * p.y + r[0][2] * p.z + r[0][3],
            r[1][0] * p.x + r[1][1] * p.y + r[1][2] * p.z + r[1][3],
            r[2][0] * p.x + r[2][1] * p.y + r[2][2] * p.z + r[2][3],
        )
    }

    pub fn vector(&self, v: Vec3) -> Vec3 {
        let r = &self.rows;
        Vec3::new(
            r[0][0] * v.x + r[0][1] * v.y + r[0][2] * v.z,
            r[1][0] * v.x + r[1][1] * v.y + r[1][2] * v.z,
            r[2][0] * v.x + r[2][1] * v.y + r[2][2] * v.z,
        )
    }
}

impl Mul for Affine {
    type Output = Self;
    fn mul(self, other: Self) -> Self {
        let a = &self.rows;
        let b = &other.rows;
        let mut rows = [[0.0; 4]; 3];
        for (i, row) in rows.iter_mut().enumerate() {
            for (j, cell) in row.iter_mut().enumerate() {
                *cell = a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j];
            }
            row[3] += a[i][3];
        }
        Self { rows }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < 1e-5
    }

    #[test]
    fn euler_matches_sequential_rotations() {
        let combined = Affine::euler_xyz(Vec3::default(), 0.3, -0.7, 1.1);
        let sequential = Affine::euler_xyz(Vec3::default(), 0.3, 0.0, 0.0)
            * Affine::euler_xyz(Vec3::default(), 0.0, -0.7, 0.0)
            * Affine::euler_xyz(Vec3::default(), 0.0, 0.0, 1.1);
        let p = Vec3::new(1.0, 2.0, 3.0);
        assert!(close(combined.point(p), sequential.point(p)));
    }

    #[test]
    fn rotation_y_turns_front_towards_plus_x() {
        let turn = Affine::euler_xyz(Vec3::default(), 0.0, std::f32::consts::FRAC_PI_2, 0.0);
        assert!(close(
            turn.vector(Vec3::new(0.0, 0.0, 1.0)),
            Vec3::new(1.0, 0.0, 0.0)
        ));
    }

    #[test]
    fn rigid_inverse_round_trips() {
        let m = Affine::euler_xyz(Vec3::new(4.0, -2.0, 9.0), 0.4, 1.2, -0.5);
        let p = Vec3::new(-3.0, 5.0, 0.5);
        assert!(close(m.inverse_rigid().point(m.point(p)), p));
        assert!(close(
            (m * Affine::translation(p)).point(Vec3::default()),
            m.point(p)
        ));
    }
}
