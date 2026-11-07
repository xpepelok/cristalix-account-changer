use std::f32::consts::TAU;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::thread;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use rodio::buffer::SamplesBuffer;
use rodio::{OutputStream, OutputStreamBuilder};

const SAMPLE_RATE: u32 = 48_000;
const MASTER_GAIN: f32 = 0.42;
const LOWPASS_HZ: f32 = 7_600.0;
const LOOKAHEAD: f32 = 0.02;
const TAP_GAP: Duration = Duration::from_millis(40);
const IDLE_CLOSE: Duration = Duration::from_secs(3);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sound {
    Tap,
    Confirm,
    Gear,
    Bell,
    Stats,
    Logs,
    Profiles,
    ThemeLight,
    ThemeDark,
}

#[derive(Clone, Copy)]
struct Blip {
    freq: f32,
    start: f32,
    dur: f32,
    peak: f32,
    attack: f32,
    glide: Option<f32>,
    detune: f32,
}

const fn blip(freq: f32, start: f32, dur: f32, peak: f32) -> Blip {
    Blip {
        freq,
        start,
        dur,
        peak,
        attack: 0.006,
        glide: None,
        detune: 0.0,
    }
}

impl Blip {
    const fn attack(self, attack: f32) -> Self {
        Self { attack, ..self }
    }

    const fn glide(self, to: f32) -> Self {
        Self {
            glide: Some(to),
            ..self
        }
    }

    const fn detune(self, cents: f32) -> Self {
        Self {
            detune: cents,
            ..self
        }
    }

    fn frequency(&self, t: f32) -> f32 {
        let base = match self.glide {
            Some(to) => {
                let span = self.dur * 0.85;
                let k = (t / span).min(1.0);
                self.freq * (to / self.freq).powf(k)
            }
            None => self.freq,
        };
        base * 2f32.powf(self.detune / 1200.0)
    }

    fn gain(&self, t: f32) -> f32 {
        const FLOOR: f32 = 0.0001;
        if t < self.attack {
            FLOOR * (self.peak / FLOOR).powf(t / self.attack)
        } else if t < self.dur {
            let k = (t - self.attack) / (self.dur - self.attack);
            self.peak * (FLOOR / self.peak).powf(k)
        } else {
            0.0
        }
    }
}

fn recipe(sound: Sound) -> Vec<Blip> {
    match sound {
        Sound::Tap => vec![blip(440.0, 0.0, 0.05, 0.04).attack(0.007).glide(380.0)],
        Sound::Confirm => vec![
            blip(523.25, 0.0, 0.13, 0.1),
            blip(783.99, 0.045, 0.17, 0.08).detune(1.0),
        ],
        Sound::Gear => vec![
            blip(392.0, 0.0, 0.05, 0.038).attack(0.007).glide(340.0),
            blip(588.0, 0.045, 0.07, 0.03).attack(0.007),
        ],
        Sound::Bell => [(1.0, 0.11), (2.01, 0.055), (3.02, 0.032), (4.24, 0.02)]
            .into_iter()
            .map(|(ratio, peak)| blip(784.0 * ratio, 0.0, 0.55 + 0.35 / ratio, peak).attack(0.004))
            .collect(),
        Sound::Stats => vec![
            blip(440.0, 0.0, 0.06, 0.032).attack(0.005),
            blip(554.37, 0.04, 0.07, 0.035).attack(0.005),
            blip(659.25, 0.08, 0.1, 0.04).attack(0.005),
        ],
        Sound::Logs => vec![
            blip(494.0, 0.0, 0.06, 0.036).attack(0.005).glide(420.0),
            blip(740.0, 0.05, 0.09, 0.03).attack(0.006),
        ],
        Sound::Profiles => vec![
            blip(494.0, 0.0, 0.16, 0.06),
            blip(622.25, 0.02, 0.18, 0.05).detune(3.0),
        ],
        Sound::ThemeLight => vec![
            blip(587.33, 0.0, 0.14, 0.09),
            blip(880.0, 0.055, 0.22, 0.08).detune(2.0),
        ],
        Sound::ThemeDark => vec![
            blip(880.0, 0.0, 0.14, 0.09),
            blip(587.33, 0.055, 0.22, 0.08),
        ],
    }
}

struct Lowpass {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl Lowpass {
    fn new(cutoff: f32, rate: f32) -> Self {
        let w0 = TAU * cutoff / rate;
        let alpha = w0.sin() / 2.0;
        let cos = w0.cos();
        let a0 = 1.0 + alpha;
        Self {
            b0: (1.0 - cos) / 2.0 / a0,
            b1: (1.0 - cos) / a0,
            b2: (1.0 - cos) / 2.0 / a0,
            a1: -2.0 * cos / a0,
            a2: (1.0 - alpha) / a0,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }

    fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2
            - self.a1 * self.y1
            - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

pub fn synthesize(sound: Sound) -> Vec<f32> {
    let blips = recipe(sound);
    let rate = SAMPLE_RATE as f32;
    let end = blips
        .iter()
        .map(|b| b.start + b.dur + 0.04)
        .fold(0.0, f32::max)
        + LOOKAHEAD;
    let mut samples = vec![0.0f32; (end * rate) as usize];
    for blip in &blips {
        let first = ((blip.start + LOOKAHEAD) * rate) as usize;
        let count = ((blip.dur + 0.04) * rate) as usize;
        let mut phase = 0.0f32;
        for (index, sample) in samples.iter_mut().skip(first).take(count).enumerate() {
            let t = index as f32 / rate;
            *sample += phase.sin() * blip.gain(t);
            phase = (phase + TAU * blip.frequency(t) / rate) % TAU;
        }
    }
    let mut filter = Lowpass::new(LOWPASS_HZ, rate);
    for sample in &mut samples {
        *sample = filter.process(*sample * MASTER_GAIN);
    }
    samples
}

static ENABLED: AtomicBool = AtomicBool::new(true);
static PLAYER: OnceLock<Mutex<Player>> = OnceLock::new();

struct Player {
    sender: Sender<Sound>,
    last_tap: Option<Instant>,
}

fn run(receiver: Receiver<Sound>) {
    let mut stream: Option<OutputStream> = None;
    loop {
        let next = if stream.is_some() {
            receiver.recv_timeout(IDLE_CLOSE)
        } else {
            receiver.recv().map_err(|_| RecvTimeoutError::Disconnected)
        };
        match next {
            Ok(sound) => {
                if stream.is_none() {
                    stream = OutputStreamBuilder::open_default_stream()
                        .ok()
                        .map(|mut opened| {
                            opened.log_on_drop(false);
                            opened
                        });
                }
                if let Some(stream) = &stream {
                    stream
                        .mixer()
                        .add(SamplesBuffer::new(1, SAMPLE_RATE, synthesize(sound)));
                }
            }
            Err(RecvTimeoutError::Timeout) => stream = None,
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

fn player() -> &'static Mutex<Player> {
    PLAYER.get_or_init(|| {
        let (sender, receiver) = channel();
        thread::Builder::new()
            .name("ui-sound".into())
            .spawn(move || run(receiver))
            .ok();
        Mutex::new(Player {
            sender,
            last_tap: None,
        })
    })
}

pub fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
}

pub fn play(sound: Sound) {
    if !ENABLED.load(Ordering::Relaxed) {
        return;
    }
    let mut player = player().lock();
    if sound == Sound::Tap {
        let now = Instant::now();
        if player.last_tap.is_some_and(|last| now - last < TAP_GAP) {
            return;
        }
        player.last_tap = Some(now);
    }
    player.sender.send(sound).ok();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sounds_are_short_and_bounded() {
        for sound in [
            Sound::Tap,
            Sound::Confirm,
            Sound::Gear,
            Sound::Bell,
            Sound::Stats,
            Sound::Logs,
            Sound::Profiles,
            Sound::ThemeLight,
            Sound::ThemeDark,
        ] {
            let samples = synthesize(sound);
            assert!(!samples.is_empty());
            assert!(samples.len() < SAMPLE_RATE as usize);
            assert!(samples.iter().all(|s| s.abs() < 1.0));
            assert!(samples.iter().any(|s| s.abs() > 1e-4));
        }
    }

    #[test]
    fn envelope_starts_quiet_and_peaks() {
        let b = blip(440.0, 0.0, 0.1, 0.1);
        assert!(b.gain(0.0) < 0.001);
        assert!((b.gain(0.006) - 0.1).abs() < 1e-3);
        assert_eq!(b.gain(0.2), 0.0);
    }
}
