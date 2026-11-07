use crate::camera::OrbitCamera;
use crate::image::PixelFormat;
use crate::math::{Affine, Vec3};
use crate::model::{Cuboid, FACES, Face, Layer, build};
use crate::pose::Pose;
use crate::texture::{Cape, Skin, Texels};

pub const DEFAULT_PLAYER_YAW: f32 = 0.5;

const NEAR: f32 = 0.1;
const DEPTH_BIAS: f32 = 1.0 - 1.0 / 65_536.0;
const MAX_POLYGON: usize = 8;

pub fn supersample_factor(device_scale: f32) -> u32 {
    let scale = device_scale.max(0.25);
    ((scale * 2.0).clamp(2.0, 4.0) / scale).round().max(1.0) as u32
}

#[derive(Debug, Clone, Copy)]
pub struct Scene<'a> {
    pub skin: &'a Skin,
    pub cape: Option<&'a Cape>,
    pub pose: Pose,
    pub player_yaw: f32,
    pub camera: OrbitCamera,
}

impl<'a> Scene<'a> {
    pub fn new(skin: &'a Skin, cape: Option<&'a Cape>) -> Self {
        Self {
            skin,
            cape,
            pose: Pose::default(),
            player_yaw: DEFAULT_PLAYER_YAW,
            camera: OrbitCamera::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Facing {
    Front,
    Back,
}

#[derive(Debug, Clone, Copy, Default)]
struct ClipVertex {
    position: Vec3,
    s: f32,
    t: f32,
}

#[derive(Debug, Clone, Copy, Default)]
struct ScreenVertex {
    x: f32,
    y: f32,
    inv_w: f32,
    s_w: f32,
    t_w: f32,
}

struct Sampler<'a> {
    texels: &'a [u32],
    stride: usize,
    width: f32,
    height: f32,
    max_x: u32,
    max_y: u32,
}

impl Sampler<'_> {
    fn new<'a>(texels: &'a Texels, data: &'a [u32], rect: [u32; 4]) -> Sampler<'a> {
        let [x, y, w, h] = rect.map(|v| v * texels.scale);
        let stride = texels.width as usize;
        Sampler {
            texels: &data[y as usize * stride + x as usize..],
            stride,
            width: w as f32,
            height: h as f32,
            max_x: w - 1,
            max_y: h - 1,
        }
    }

    #[inline(always)]
    fn sample(&self, u: f32, v: f32) -> u32 {
        let x = (u as u32).min(self.max_x) as usize;
        let y = (v as u32).min(self.max_y) as usize;
        self.texels[y * self.stride + x]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Bounds {
    min_x: usize,
    min_y: usize,
    max_x: usize,
    max_y: usize,
}

impl Bounds {
    const EMPTY: Self = Self {
        min_x: usize::MAX,
        min_y: usize::MAX,
        max_x: 0,
        max_y: 0,
    };

    fn include_span(&mut self, row: usize, first: usize, end: usize) {
        self.min_x = self.min_x.min(first);
        self.max_x = self.max_x.max(end);
        self.min_y = self.min_y.min(row);
        self.max_y = self.max_y.max(row + 1);
    }

    fn is_empty(&self) -> bool {
        self.min_x >= self.max_x || self.min_y >= self.max_y
    }
}

struct Target<'a> {
    color: &'a mut [u32],
    depth: &'a mut [f32],
    width: usize,
    height: usize,
    touched: Bounds,
}

struct Material<'a> {
    sampler: Sampler<'a>,
    blended: bool,
    bias: f32,
}

#[derive(Default)]
pub struct SkinRenderer {
    width: u32,
    height: u32,
    supersample: u32,
    color: Vec<u32>,
    depth: Vec<f32>,
    output: Vec<u8>,
    cuboids: Vec<Cuboid>,
    order: Vec<(u8, f32, usize)>,
    dirty: Option<Bounds>,
}

impl SkinRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn render(
        &mut self,
        scene: &Scene<'_>,
        width: u32,
        height: u32,
        supersample: u32,
        format: PixelFormat,
    ) -> &[u8] {
        let supersample = supersample.max(1);
        self.resize(width, height, supersample);
        self.clear_dirty();
        self.output.fill(0);
        if width == 0 || height == 0 {
            return &self.output;
        }

        build(
            scene.skin.model(),
            &scene.pose,
            scene.player_yaw,
            scene.cape.is_some(),
            &mut self.cuboids,
        );
        let view = scene.camera.view();
        let internal_width = (width * supersample) as usize;
        let internal_height = (height * supersample) as usize;
        let focal = internal_height as f32 * 0.5 / (scene.camera.fov().to_radians() * 0.5).tan();
        let projection = Projection {
            focal,
            center_x: internal_width as f32 * 0.5,
            center_y: internal_height as f32 * 0.5,
        };

        self.order.clear();
        self.order
            .extend(self.cuboids.iter().enumerate().map(|(index, cuboid)| {
                let distance = -(view * cuboid.transform).point(Vec3::default()).z;
                let group = match (cuboid.layer, cuboid.biased) {
                    (Layer::Inner, false) => 0,
                    (Layer::Inner, true) => 1,
                    _ => 2,
                };
                (group, distance, index)
            }));
        self.order.sort_by(|a, b| {
            a.0.cmp(&b.0).then_with(|| {
                if a.0 == 2 {
                    b.1.total_cmp(&a.1)
                } else {
                    a.1.total_cmp(&b.1)
                }
            })
        });

        let mut target = Target {
            color: &mut self.color,
            depth: &mut self.depth,
            width: internal_width,
            height: internal_height,
            touched: Bounds::EMPTY,
        };
        for &(_, _, index) in &self.order {
            let cuboid = &self.cuboids[index];
            let model_view = view * cuboid.transform;
            let (texels, blended) = match (cuboid.layer, scene.cape) {
                (Layer::Inner, _) => (scene.skin.texels(), false),
                (Layer::Outer, _) => (scene.skin.texels(), true),
                (Layer::Cape, Some(cape)) => (cape.texels(), true),
                (Layer::Cape, None) => continue,
            };
            let data = if blended {
                &texels.blended
            } else {
                &texels.opaque
            };
            let passes: &[Facing] = if blended {
                &[Facing::Back, Facing::Front]
            } else {
                &[Facing::Front]
            };
            for &pass in passes {
                for face in &FACES {
                    let material = Material {
                        sampler: Sampler::new(texels, data, face.rect(cuboid.uv)),
                        blended,
                        bias: if cuboid.biased { DEPTH_BIAS } else { 1.0 },
                    };
                    draw_face(
                        &mut target,
                        &projection,
                        &model_view.scaled(cuboid.half),
                        face,
                        pass,
                        &material,
                    );
                }
            }
        }

        let touched = target.touched;
        self.dirty = Some(touched);
        if !touched.is_empty() {
            resolve(
                &self.color,
                internal_width,
                supersample as usize,
                &mut self.output,
                width as usize,
                touched,
                format,
            );
        }
        &self.output
    }

    fn clear_dirty(&mut self) {
        let stride = (self.width * self.supersample) as usize;
        match self.dirty.take() {
            None => {
                self.color.fill(0);
                self.depth.fill(0.0);
            }
            Some(bounds) if !bounds.is_empty() => {
                for row in bounds.min_y..bounds.max_y {
                    let span = row * stride + bounds.min_x..row * stride + bounds.max_x;
                    self.color[span.clone()].fill(0);
                    self.depth[span].fill(0.0);
                }
            }
            Some(_) => {}
        }
    }

    fn resize(&mut self, width: u32, height: u32, supersample: u32) {
        if (self.width, self.height, self.supersample) == (width, height, supersample) {
            return;
        }
        let samples = (width * supersample) as usize * (height * supersample) as usize;
        self.color.resize(samples, 0);
        self.depth.resize(samples, 0.0);
        self.output.resize(width as usize * height as usize * 4, 0);
        self.dirty = None;
        (self.width, self.height, self.supersample) = (width, height, supersample);
    }
}

struct Projection {
    focal: f32,
    center_x: f32,
    center_y: f32,
}

fn draw_face(
    target: &mut Target<'_>,
    projection: &Projection,
    model_view: &Affine,
    face: &Face,
    pass: Facing,
    material: &Material<'_>,
) {
    let origin = model_view.point(face.origin);
    let s_axis = model_view.vector(face.s_axis);
    let t_axis = model_view.vector(face.t_axis);
    let facing = if model_view.vector(face.normal).dot(origin) < 0.0 {
        Facing::Front
    } else {
        Facing::Back
    };
    if facing != pass {
        return;
    }

    let corners = [
        ClipVertex {
            position: origin,
            s: 0.0,
            t: 0.0,
        },
        ClipVertex {
            position: origin + t_axis,
            s: 0.0,
            t: 1.0,
        },
        ClipVertex {
            position: origin + s_axis + t_axis,
            s: 1.0,
            t: 1.0,
        },
        ClipVertex {
            position: origin + s_axis,
            s: 1.0,
            t: 0.0,
        },
    ];
    let mut clipped = [ClipVertex::default(); MAX_POLYGON];
    let count = clip_near(&corners, &mut clipped);
    if count < 3 {
        return;
    }
    let mut screen = [ScreenVertex::default(); MAX_POLYGON];
    for (out, vertex) in screen.iter_mut().zip(&clipped[..count]) {
        let inv_w = 1.0 / -vertex.position.z;
        *out = ScreenVertex {
            x: projection.center_x + vertex.position.x * inv_w * projection.focal,
            y: projection.center_y - vertex.position.y * inv_w * projection.focal,
            inv_w,
            s_w: vertex.s * material.sampler.width * inv_w,
            t_w: vertex.t * material.sampler.height * inv_w,
        };
    }
    fill(target, &screen[..count], material);
}

fn clip_near(input: &[ClipVertex; 4], output: &mut [ClipVertex; MAX_POLYGON]) -> usize {
    let limit = -NEAR;
    let mut count = 0;
    for (index, &current) in input.iter().enumerate() {
        let next = input[(index + 1) % input.len()];
        let current_inside = current.position.z <= limit;
        let next_inside = next.position.z <= limit;
        if current_inside {
            output[count] = current;
            count += 1;
        }
        if current_inside != next_inside {
            let t = (limit - current.position.z) / (next.position.z - current.position.z);
            output[count] = ClipVertex {
                position: current.position.lerp(next.position, t),
                s: current.s + (next.s - current.s) * t,
                t: current.t + (next.t - current.t) * t,
            };
            count += 1;
        }
    }
    count
}

#[derive(Clone, Copy)]
struct Plane {
    base: f32,
    dx: f32,
    dy: f32,
}

impl Plane {
    fn fit(
        v0: &ScreenVertex,
        v1: &ScreenVertex,
        v2: &ScreenVertex,
        value: fn(&ScreenVertex) -> f32,
    ) -> Self {
        let (ax, ay) = (v1.x - v0.x, v1.y - v0.y);
        let (bx, by) = (v2.x - v0.x, v2.y - v0.y);
        let det = ax * by - bx * ay;
        let (da, db) = (value(v1) - value(v0), value(v2) - value(v0));
        let dx = (da * by - db * ay) / det;
        let dy = (db * ax - da * bx) / det;
        Self {
            base: value(v0) - dx * v0.x - dy * v0.y,
            dx,
            dy,
        }
    }

    #[inline(always)]
    fn at(&self, x: f32, y: f32) -> f32 {
        self.base + self.dx * x + self.dy * y
    }
}

fn fill(target: &mut Target<'_>, polygon: &[ScreenVertex], material: &Material<'_>) {
    let &Material {
        ref sampler,
        blended,
        bias,
    } = material;
    let count = polygon.len();
    let twice_area: f32 = (0..count)
        .map(|i| {
            let (a, b) = (&polygon[i], &polygon[(i + 1) % count]);
            a.x * b.y - b.x * a.y
        })
        .sum();
    if twice_area.abs() < 1e-6 {
        return;
    }
    let orientation = twice_area.signum();

    let anchor = (1..count - 1)
        .max_by(|&i, &j| {
            triangle_area(&polygon[0], &polygon[i], &polygon[i + 1]).total_cmp(&triangle_area(
                &polygon[0],
                &polygon[j],
                &polygon[j + 1],
            ))
        })
        .unwrap_or(1);
    let (v0, v1, v2) = (&polygon[0], &polygon[anchor], &polygon[anchor + 1]);
    let inv_w = Plane::fit(v0, v1, v2, |v| v.inv_w);
    let s_w = Plane::fit(v0, v1, v2, |v| v.s_w);
    let t_w = Plane::fit(v0, v1, v2, |v| v.t_w);

    let (min_y, max_y) = polygon
        .iter()
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), v| {
            (lo.min(v.y), hi.max(v.y))
        });
    let first_row = (min_y - 0.5).ceil().max(0.0) as usize;
    let end_row = ((max_y - 0.5).ceil().max(0.0) as usize).min(target.height);

    for row in first_row..end_row {
        let yc = row as f32 + 0.5;
        let mut lo = 0.0f32;
        let mut hi = target.width as f32;
        let mut empty = false;
        for i in 0..count {
            let (a, b) = (&polygon[i], &polygon[(i + 1) % count]);
            let slope = -orientation * (b.y - a.y);
            let offset = orientation * ((b.x - a.x) * (yc - a.y) + (b.y - a.y) * a.x);
            if slope > 0.0 {
                lo = lo.max(-offset / slope);
            } else if slope < 0.0 {
                hi = hi.min(-offset / slope);
            } else if offset < 0.0 {
                empty = true;
                break;
            }
        }
        if empty {
            continue;
        }
        let first = (lo - 0.5).ceil().max(0.0) as usize;
        let end = ((hi - 0.5).ceil().max(0.0) as usize).min(target.width);
        if first >= end {
            continue;
        }

        target.touched.include_span(row, first, end);
        let xc = first as f32 + 0.5;
        let mut w = inv_w.at(xc, yc);
        let mut sw = s_w.at(xc, yc);
        let mut tw = t_w.at(xc, yc);
        let row_offset = row * target.width;
        let colors = &mut target.color[row_offset + first..row_offset + end];
        let depths = &mut target.depth[row_offset + first..row_offset + end];
        for (color, depth) in colors.iter_mut().zip(depths.iter_mut()) {
            let biased = w * bias;
            if biased >= *depth {
                let reciprocal = 1.0 / w;
                let texel = sampler.sample(sw * reciprocal, tw * reciprocal);
                if !blended {
                    *color = texel;
                    *depth = biased;
                } else if texel >> 24 != 0 {
                    *color = over(texel, *color);
                    *depth = biased;
                }
            }
            w += inv_w.dx;
            sw += s_w.dx;
            tw += t_w.dx;
        }
    }
}

fn triangle_area(a: &ScreenVertex, b: &ScreenVertex, c: &ScreenVertex) -> f32 {
    ((b.x - a.x) * (c.y - a.y) - (c.x - a.x) * (b.y - a.y)).abs()
}

#[inline(always)]
fn over(source: u32, destination: u32) -> u32 {
    let alpha = source >> 24;
    if alpha == 255 {
        return source;
    }
    let keep = 255 - alpha;
    let channel = |shift: u32| {
        let src = (source >> shift) & 0xff;
        let dst = (destination >> shift) & 0xff;
        (src + (dst * keep + 127) / 255).min(255) << shift
    };
    channel(0) | channel(8) | channel(16) | channel(24)
}

fn resolve(
    color: &[u32],
    stride: usize,
    factor: usize,
    output: &mut [u8],
    width: usize,
    touched: Bounds,
    format: PixelFormat,
) {
    let samples = (factor * factor) as u32;
    let (pixels, _) = output.as_chunks_mut::<4>();
    let height = pixels.len() / width;
    let columns = touched.min_x / factor..touched.max_x.div_ceil(factor).min(width);
    for y in touched.min_y / factor..touched.max_y.div_ceil(factor).min(height) {
        for x in columns.clone() {
            let mut sum = [0u32; 4];
            for sy in 0..factor {
                let row = (y * factor + sy) * stride + x * factor;
                for &sample in &color[row..row + factor] {
                    sum[0] += sample & 0xff;
                    sum[1] += (sample >> 8) & 0xff;
                    sum[2] += (sample >> 16) & 0xff;
                    sum[3] += sample >> 24;
                }
            }
            let alpha = sum[3];
            if alpha == 0 {
                continue;
            }
            let straight = |channel: u32| ((channel * 255 + alpha / 2) / alpha).min(255) as u8;
            let rgba = [
                straight(sum[0]),
                straight(sum[1]),
                straight(sum[2]),
                ((alpha + samples / 2) / samples) as u8,
            ];
            let offset = y * width + x;
            format.write(&mut pixels[offset], rgba);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::RgbaImage;

    fn solid_skin(color: [u8; 4]) -> Skin {
        let mut image = RgbaImage::new(64, 64);
        image.pixels.fill(color);
        for y in 8..16 {
            for x in 8..16 {
                image.set(x, y, [255, 0, 0, 255]);
            }
        }
        Skin::from_image(image).unwrap()
    }

    fn pixel(frame: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * width + x) * 4) as usize;
        [frame[i], frame[i + 1], frame[i + 2], frame[i + 3]]
    }

    #[test]
    fn supersample_matches_skinview_pixel_ratio() {
        assert_eq!(supersample_factor(1.0), 2);
        assert_eq!(supersample_factor(1.5), 2);
        assert_eq!(supersample_factor(2.0), 2);
        assert_eq!(supersample_factor(3.0), 1);
    }

    #[test]
    fn front_view_shows_lit_face_and_transparent_corners() {
        let skin = solid_skin([200, 200, 200, 255]);
        let mut scene = Scene::new(&skin, None);
        scene.player_yaw = 0.0;
        let mut renderer = SkinRenderer::new();
        let frame = renderer
            .render(&scene, 200, 400, 2, PixelFormat::Rgba)
            .to_vec();
        assert_eq!(frame.len(), 200 * 400 * 4);
        assert_eq!(pixel(&frame, 200, 0, 0), [0; 4]);
        assert_eq!(pixel(&frame, 200, 100, 90), [244, 0, 0, 255]);
        assert_eq!(pixel(&frame, 200, 100, 200), [191, 191, 191, 255]);
        let bgra = renderer.render(&scene, 200, 400, 2, PixelFormat::Bgra);
        assert_eq!(pixel(bgra, 200, 100, 90), [0, 0, 244, 255]);
    }

    #[test]
    fn silhouette_is_antialiased() {
        let skin = solid_skin([200, 200, 200, 255]);
        let scene = Scene::new(&skin, None);
        let mut renderer = SkinRenderer::new();
        let frame = renderer.render(&scene, 210, 320, 2, PixelFormat::Rgba);
        let partial = frame
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[3] != 0 && p[3] != 255)
            .count();
        assert!(partial > 50, "{partial}");
    }

    #[test]
    fn extreme_camera_angles_do_not_panic() {
        let skin = solid_skin([200, 200, 200, 255]);
        let mut scene = Scene::new(&skin, None);
        let mut renderer = SkinRenderer::new();
        for _ in 0..200 {
            scene.camera.wheel(-1.0);
        }
        for step in 0..16 {
            scene.camera.drag(37.0, 23.0 * (step as f32 - 8.0), 300.0);
            renderer.render(&scene, 120, 160, 1, PixelFormat::Rgba);
        }
        renderer.render(&scene, 0, 0, 2, PixelFormat::Rgba);
    }
}
