use std::io::{BufRead, BufReader, Read};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use ac_api::VideoBackground;
use futures::channel::mpsc as channel;
use gpui::RenderImage;
use image::{Frame, RgbaImage};
use parking_lot::Mutex;
use smallvec::smallvec;

pub const WIDTH: u32 = 1280;
pub const HEIGHT: u32 = 720;
const MAX_FPS: f64 = 30.0;
const FPS_WAIT: Duration = Duration::from_secs(5);
const PAUSE_POLL: Duration = Duration::from_millis(120);

pub struct VideoPlayer {
    live: Arc<AtomicBool>,
    child: Arc<Mutex<Option<Child>>>,
}

impl VideoPlayer {
    pub fn start(
        source: &VideoBackground,
        blur: u8,
    ) -> std::io::Result<(Self, channel::Receiver<Arc<RenderImage>>)> {
        let mut command = Command::new(&source.ffmpeg);
        command
            .args(arguments(&source.file.to_string_lossy(), blur))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000);
        }
        let mut child = command.spawn()?;
        let stdout = child.stdout.take().ok_or(std::io::ErrorKind::BrokenPipe)?;
        let stderr = child.stderr.take().ok_or(std::io::ErrorKind::BrokenPipe)?;
        let live = Arc::new(AtomicBool::new(true));
        let (fps_tx, fps_rx) = mpsc::channel();
        let (frames_tx, frames_rx) = channel::channel(1);
        std::thread::Builder::new()
            .name("video-log".into())
            .spawn(move || watch_log(stderr, fps_tx))?;
        let reader_live = live.clone();
        std::thread::Builder::new()
            .name("video-frames".into())
            .spawn(move || pump(stdout, fps_rx, frames_tx, reader_live))?;
        Ok((
            Self {
                live,
                child: Arc::new(Mutex::new(Some(child))),
            },
            frames_rx,
        ))
    }

    pub fn set_live(&self, live: bool) {
        self.live.store(live, Ordering::Relaxed);
    }
}

impl Drop for VideoPlayer {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.lock().take() {
            child.kill().ok();
            child.wait().ok();
        }
    }
}

fn arguments(file: &str, blur: u8) -> Vec<String> {
    let mut filter = format!(
        "scale={WIDTH}:{HEIGHT}:force_original_aspect_ratio=increase:flags=fast_bilinear,crop={WIDTH}:{HEIGHT}"
    );
    if blur > 0 {
        filter.push_str(&format!(",gblur=sigma={}", f32::from(blur) * 0.5));
    }
    [
        "-hide_banner",
        "-nostdin",
        "-nostats",
        "-loglevel",
        "info",
        "-hwaccel",
        "auto",
        "-stream_loop",
        "-1",
        "-i",
        file,
        "-an",
        "-sn",
        "-dn",
        "-vf",
        &filter,
        "-fpsmax",
        "30",
        "-pix_fmt",
        "bgra",
        "-f",
        "rawvideo",
        "pipe:1",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

fn parse_fps(line: &str) -> Option<f64> {
    let head = &line[..line.find(" fps")?];
    let value = head.rsplit([' ', ',']).next()?;
    value.parse::<f64>().ok().filter(|fps| *fps > 0.0)
}

fn watch_log(stderr: impl Read, fps: mpsc::Sender<f64>) {
    let mut output = false;
    let mut sent = false;
    for line in BufReader::new(stderr).lines() {
        let Ok(line) = line else {
            break;
        };
        if line.starts_with("Output #0") {
            output = true;
        }
        if output
            && !sent
            && let Some(value) = parse_fps(&line)
        {
            fps.send(value).ok();
            sent = true;
        }
    }
}

fn pump(
    mut stdout: impl Read,
    fps: mpsc::Receiver<f64>,
    mut frames: channel::Sender<Arc<RenderImage>>,
    live: Arc<AtomicBool>,
) {
    let size = (WIDTH * HEIGHT * 4) as usize;
    let mut fps_value = None;
    let mut next = Instant::now();
    loop {
        if !live.load(Ordering::Relaxed) {
            std::thread::sleep(PAUSE_POLL);
            next = Instant::now();
            continue;
        }
        let mut buffer = vec![0; size];
        if stdout.read_exact(&mut buffer).is_err() {
            break;
        }
        let rate = *fps_value
            .get_or_insert_with(|| fps.recv_timeout(FPS_WAIT).unwrap_or(MAX_FPS).min(MAX_FPS));
        let Some(image) = RgbaImage::from_raw(WIDTH, HEIGHT, buffer) else {
            break;
        };
        let frame = Arc::new(RenderImage::new(smallvec![Frame::new(image)]));
        match frames.try_send(frame) {
            Ok(()) => {}
            Err(error) if error.is_disconnected() => break,
            Err(_) => {}
        }
        next += Duration::from_secs_f64(1.0 / rate);
        let now = Instant::now();
        if next > now {
            std::thread::sleep(next - now);
        } else if now - next > Duration::from_millis(250) {
            next = now;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_output_fps() {
        assert_eq!(
            parse_fps(
                "  Stream #0:0: Video: rawvideo (BGRA / 0x41524742), bgra(pc, gbr/unknown/unknown, progressive), 1280x720, q=2-31, 884736 kb/s, 29.97 fps, 29.97 tbn"
            ),
            Some(29.97)
        );
        assert_eq!(parse_fps("Output #0, rawvideo, to 'pipe:1':"), None);
    }

    #[test]
    fn builds_filter_with_blur() {
        let args = arguments("a.mp4", 10);
        let filter = &args[args.iter().position(|arg| arg == "-vf").unwrap() + 1];
        assert!(filter.ends_with("gblur=sigma=5"));
        let plain = arguments("a.mp4", 0);
        let filter = &plain[plain.iter().position(|arg| arg == "-vf").unwrap() + 1];
        assert!(!filter.contains("gblur"));
    }
}
