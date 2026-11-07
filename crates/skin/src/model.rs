use std::f32::consts::PI;

use crate::math::{Affine, Vec3};
use crate::pose::Pose;
use crate::texture::ModelType;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Layer {
    Inner,
    Outer,
    Cape,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Cuboid {
    pub transform: Affine,
    pub half: Vec3,
    pub uv: UvBox,
    pub layer: Layer,
    pub biased: bool,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct UvBox {
    pub u: u32,
    pub v: u32,
    pub width: u32,
    pub height: u32,
    pub depth: u32,
}

pub(crate) struct Face {
    pub origin: Vec3,
    pub s_axis: Vec3,
    pub t_axis: Vec3,
    pub normal: Vec3,
    rect: fn(UvBox) -> [u32; 4],
}

impl Face {
    pub fn rect(&self, uv: UvBox) -> [u32; 4] {
        (self.rect)(uv)
    }
}

pub(crate) const FACES: [Face; 6] = [
    Face {
        origin: Vec3::new(1.0, 1.0, 1.0),
        s_axis: Vec3::new(0.0, 0.0, -2.0),
        t_axis: Vec3::new(0.0, -2.0, 0.0),
        normal: Vec3::new(1.0, 0.0, 0.0),
        rect: |b| [b.u + b.width + b.depth, b.v + b.depth, b.depth, b.height],
    },
    Face {
        origin: Vec3::new(-1.0, 1.0, -1.0),
        s_axis: Vec3::new(0.0, 0.0, 2.0),
        t_axis: Vec3::new(0.0, -2.0, 0.0),
        normal: Vec3::new(-1.0, 0.0, 0.0),
        rect: |b| [b.u, b.v + b.depth, b.depth, b.height],
    },
    Face {
        origin: Vec3::new(-1.0, 1.0, -1.0),
        s_axis: Vec3::new(2.0, 0.0, 0.0),
        t_axis: Vec3::new(0.0, 0.0, 2.0),
        normal: Vec3::new(0.0, 1.0, 0.0),
        rect: |b| [b.u + b.depth, b.v, b.width, b.depth],
    },
    Face {
        origin: Vec3::new(-1.0, -1.0, -1.0),
        s_axis: Vec3::new(2.0, 0.0, 0.0),
        t_axis: Vec3::new(0.0, 0.0, 2.0),
        normal: Vec3::new(0.0, -1.0, 0.0),
        rect: |b| [b.u + b.depth + b.width, b.v, b.width, b.depth],
    },
    Face {
        origin: Vec3::new(-1.0, 1.0, 1.0),
        s_axis: Vec3::new(2.0, 0.0, 0.0),
        t_axis: Vec3::new(0.0, -2.0, 0.0),
        normal: Vec3::new(0.0, 0.0, 1.0),
        rect: |b| [b.u + b.depth, b.v + b.depth, b.width, b.height],
    },
    Face {
        origin: Vec3::new(1.0, 1.0, -1.0),
        s_axis: Vec3::new(-2.0, 0.0, 0.0),
        t_axis: Vec3::new(0.0, -2.0, 0.0),
        normal: Vec3::new(0.0, 0.0, -1.0),
        rect: |b| {
            [
                b.u + 2 * b.depth + b.width,
                b.v + b.depth,
                b.width,
                b.height,
            ]
        },
    },
];

struct Part {
    joint: Affine,
    inner: (Vec3, UvBox),
    outer: (Vec3, UvBox),
    biased: bool,
}

const fn uv(u: u32, v: u32, width: u32, height: u32, depth: u32) -> UvBox {
    UvBox {
        u,
        v,
        width,
        height,
        depth,
    }
}

fn layers(size: Vec3, inflate: f32) -> (Vec3, Vec3) {
    let half = size * 0.5;
    (half, half + Vec3::new(inflate, inflate, inflate) * 0.5)
}

pub(crate) fn build(
    model: ModelType,
    pose: &Pose,
    player_yaw: f32,
    cape: bool,
    out: &mut Vec<Cuboid>,
) {
    out.clear();
    let player = Affine::euler_xyz(Vec3::default(), 0.0, player_yaw, 0.0);
    let skin = player * Affine::translation(Vec3::new(0.0, 8.0, 0.0));
    let slim = model == ModelType::Slim;
    let arm_width = if slim { 3 } else { 4 };
    let arm_shift = if slim { 0.5 } else { 1.0 };

    let (head_inner, head_outer) = layers(Vec3::new(8.0, 8.0, 8.0), 1.0);
    let (body_inner, body_outer) = layers(Vec3::new(8.0, 12.0, 4.0), 0.5);
    let (arm_inner, arm_outer) = layers(Vec3::new(arm_width as f32, 12.0, 4.0), 0.5);
    let (leg_inner, leg_outer) = layers(Vec3::new(4.0, 12.0, 4.0), 0.5);
    let limb = |position: Vec3, roll: f32, offset: Vec3| {
        skin * Affine::euler_xyz(position, 0.0, 0.0, roll) * Affine::translation(offset)
    };

    let parts = [
        Part {
            joint: skin * Affine::translation(Vec3::new(0.0, 4.0, 0.0)),
            inner: (head_inner, uv(0, 0, 8, 8, 8)),
            outer: (head_outer, uv(32, 0, 8, 8, 8)),
            biased: false,
        },
        Part {
            joint: skin * Affine::translation(Vec3::new(0.0, -6.0, 0.0)),
            inner: (body_inner, uv(16, 16, 8, 12, 4)),
            outer: (body_outer, uv(16, 32, 8, 12, 4)),
            biased: false,
        },
        Part {
            joint: limb(
                Vec3::new(-5.0, -2.0, 0.0),
                pose.right_arm_roll,
                Vec3::new(-arm_shift, -4.0, 0.0),
            ),
            inner: (arm_inner, uv(40, 16, arm_width, 12, 4)),
            outer: (arm_outer, uv(40, 32, arm_width, 12, 4)),
            biased: true,
        },
        Part {
            joint: limb(
                Vec3::new(5.0, -2.0, 0.0),
                pose.left_arm_roll,
                Vec3::new(arm_shift, -4.0, 0.0),
            ),
            inner: (arm_inner, uv(32, 48, arm_width, 12, 4)),
            outer: (arm_outer, uv(48, 48, arm_width, 12, 4)),
            biased: true,
        },
        Part {
            joint: limb(Vec3::new(-1.9, -12.0, -0.1), 0.0, Vec3::new(0.0, -6.0, 0.0)),
            inner: (leg_inner, uv(0, 16, 4, 12, 4)),
            outer: (leg_outer, uv(0, 32, 4, 12, 4)),
            biased: true,
        },
        Part {
            joint: limb(Vec3::new(1.9, -12.0, -0.1), 0.0, Vec3::new(0.0, -6.0, 0.0)),
            inner: (leg_inner, uv(16, 48, 4, 12, 4)),
            outer: (leg_outer, uv(0, 48, 4, 12, 4)),
            biased: true,
        },
    ];

    for part in parts {
        for (layer, (half, uv)) in [(Layer::Inner, part.inner), (Layer::Outer, part.outer)] {
            out.push(Cuboid {
                transform: part.joint,
                half,
                uv,
                layer,
                biased: part.biased,
            });
        }
    }

    if cape {
        out.push(Cuboid {
            transform: player
                * Affine::euler_xyz(Vec3::new(0.0, 8.0, -2.0), pose.cape_pitch, PI, 0.0)
                * Affine::translation(Vec3::new(0.0, -8.0, 0.5)),
            half: Vec3::new(5.0, 8.0, 0.5),
            uv: uv(0, 0, 10, 16, 1),
            layer: Layer::Cape,
            biased: false,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn faces_tile_the_minecraft_box_layout() {
        let head = uv(0, 0, 8, 8, 8);
        let rects: Vec<_> = FACES.iter().map(|face| face.rect(head)).collect();
        assert_eq!(
            rects,
            [
                [16, 8, 8, 8],
                [0, 8, 8, 8],
                [8, 0, 8, 8],
                [16, 0, 8, 8],
                [8, 8, 8, 8],
                [24, 8, 8, 8]
            ]
        );
    }

    #[test]
    fn face_axes_span_the_unit_cube() {
        for face in &FACES {
            let corners = [
                face.origin,
                face.origin + face.s_axis,
                face.origin + face.t_axis,
                face.origin + face.s_axis + face.t_axis,
            ];
            for corner in corners {
                assert!(
                    [corner.x, corner.y, corner.z]
                        .iter()
                        .all(|c| c.abs() == 1.0)
                );
                assert_eq!(corner.dot(face.normal), 1.0);
            }
        }
    }

    #[test]
    fn rest_model_spans_minus_16_to_16() {
        let mut cuboids = Vec::new();
        build(
            ModelType::Classic,
            &Pose::default(),
            0.0,
            true,
            &mut cuboids,
        );
        assert_eq!(cuboids.len(), 13);
        let extent = |sign: f32| {
            cuboids
                .iter()
                .filter(|c| c.layer == Layer::Inner)
                .map(|c| c.transform.point(Vec3::new(0.0, sign * c.half.y, 0.0)).y)
                .fold(-sign * f32::INFINITY, |acc, y| {
                    if sign > 0.0 { acc.max(y) } else { acc.min(y) }
                })
        };
        assert!((extent(1.0) - 16.0).abs() < 1e-4);
        assert!((extent(-1.0) + 16.0).abs() < 1e-4);
    }

    #[test]
    fn slim_arms_touch_the_body() {
        let mut cuboids = Vec::new();
        build(ModelType::Slim, &Pose::default(), 0.0, false, &mut cuboids);
        let right_arm = cuboids[4];
        let inner_edge = right_arm
            .transform
            .point(Vec3::new(right_arm.half.x, 0.0, 0.0));
        assert!((inner_edge.x + 4.0).abs() < 1e-5);
        assert_eq!(right_arm.half.x, 1.5);
    }
}
