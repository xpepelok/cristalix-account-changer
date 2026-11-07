use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use ac_skin::{Cape, PixelFormat, Pose, Scene, Skin, SkinRenderer};

const WIDTH: u32 = 210;
const HEIGHT: u32 = 320;
const BENCH_WIDTH: u32 = 300;
const BENCH_HEIGHT: u32 = 400;
const BENCH_FRAMES: u32 = 240;

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os().skip(1).map(PathBuf::from);
    let input = args.next().ok_or("usage: render <skins-dir> <out-dir>")?;
    let output = args
        .next()
        .unwrap_or_else(|| std::env::temp_dir().join("ac-skin-render"));
    fs::create_dir_all(&output)?;

    let mut renderer = SkinRenderer::new();
    for entry in fs::read_dir(&input)? {
        let path = entry?.path();
        let Some(id) = path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_prefix("skin-"))
            .and_then(|name| name.strip_suffix(".png"))
        else {
            continue;
        };
        let skin = Skin::from_png(&fs::read(&path)?)?;
        let cape = fs::read(input.join(format!("cape-{id}.png")))
            .ok()
            .map(|bytes| Cape::from_png(&bytes))
            .transpose()?;

        let mut scene = Scene::new(&skin, cape.as_ref());
        scene.pose = Pose::idle(0.0);
        let views = [
            ("front", 0.0, 0.0),
            ("back", 160.0, 0.0),
            ("above", 40.0, 60.0),
        ];
        for (name, dx, dy) in views {
            let mut view = scene;
            view.camera.drag(dx, dy, HEIGHT as f32);
            let frame = renderer.render(&view, WIDTH, HEIGHT, 2, PixelFormat::Rgba);
            write_png(
                &output.join(format!("{id}-{name}.png")),
                WIDTH,
                HEIGHT,
                frame,
            )?;
        }
        for size in [26, 46, 128] {
            let head = skin.head_avatar(size, PixelFormat::Rgba);
            write_png(
                &output.join(format!("{id}-head{size}.png")),
                size,
                size,
                &head,
            )?;
        }

        let started = Instant::now();
        for frame in 0..BENCH_FRAMES {
            scene.pose = Pose::idle(frame as f32 / 60.0);
            scene.camera.drag(1.0, 0.0, BENCH_HEIGHT as f32);
            renderer.render(&scene, BENCH_WIDTH, BENCH_HEIGHT, 2, PixelFormat::Bgra);
        }
        let per_frame = started.elapsed().as_secs_f64() * 1000.0 / f64::from(BENCH_FRAMES);
        println!(
            "{id} {:?} cape={} {per_frame:.3} ms/frame",
            skin.model(),
            cape.is_some()
        );
    }
    println!("{}", output.display());
    Ok(())
}

fn write_png(path: &Path, width: u32, height: u32, rgba: &[u8]) -> Result<(), Box<dyn Error>> {
    let mut encoder = png::Encoder::new(fs::File::create(path)?, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(rgba)?;
    writer.finish()?;
    Ok(())
}
