use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::time::{Duration, Instant};

use gpui::{App, Global, Window};

pub const HOVER: Duration = Duration::from_millis(150);

pub const EASE: [f32; 4] = [0.25, 0.1, 0.25, 1.0];
pub const EASE_OUT: [f32; 4] = [0.22, 0.61, 0.36, 1.0];
pub const EASE_SPRING: [f32; 4] = [0.34, 1.4, 0.6, 1.0];
pub const EASE_IN_OUT: [f32; 4] = [0.42, 0.0, 0.58, 1.0];
pub const EASE_IN: [f32; 4] = [0.4, 0.0, 1.0, 1.0];
pub const STANDARD: [f32; 4] = [0.4, 0.0, 0.2, 1.0];
pub const LINEAR: [f32; 4] = [0.0, 0.0, 1.0, 1.0];

pub fn keyframes(stops: &[(f32, f32)], t: f32, curve: [f32; 4]) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let Some(last) = stops.last() else {
        return 0.0;
    };
    for pair in stops.windows(2) {
        let ((t0, v0), (t1, v1)) = (pair[0], pair[1]);
        if t <= t1 {
            let local = if t1 > t0 { (t - t0) / (t1 - t0) } else { 1.0 };
            return v0 + (v1 - v0) * cubic_bezier(curve, local);
        }
    }
    last.1
}

pub fn cubic_bezier(curve: [f32; 4], t: f32) -> f32 {
    let [x1, y1, x2, y2] = curve;
    let t = t.clamp(0.0, 1.0);
    let bezier = |a: f32, b: f32, u: f32| {
        let v = 1.0 - u;
        3.0 * v * v * u * a + 3.0 * v * u * u * b + u * u * u
    };
    let (mut low, mut high, mut u) = (0.0_f32, 1.0_f32, t);
    for _ in 0..20 {
        if bezier(x1, x2, u) < t {
            low = u;
        } else {
            high = u;
        }
        u = (low + high) * 0.5;
    }
    bezier(y1, y2, u)
}

pub fn eased(curve: [f32; 4]) -> impl Fn(f32) -> f32 {
    move |t| cubic_bezier(curve, t).clamp(0.0, 1.0)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Key(u64);

impl Key {
    pub fn new(scope: &'static str, id: impl Hash) -> Self {
        let mut hasher = DefaultHasher::new();
        scope.hash(&mut hasher);
        id.hash(&mut hasher);
        Self(hasher.finish())
    }

    pub const fn of(scope: &'static str) -> Self {
        let bytes = scope.as_bytes();
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        let mut index = 0;
        while index < bytes.len() {
            hash ^= bytes[index] as u64;
            hash = hash.wrapping_mul(0x0100_0000_01b3);
            index += 1;
        }
        Self(hash)
    }
}

#[derive(Clone, Copy, Debug)]
struct Track {
    from: f32,
    to: f32,
    started: Instant,
    duration: Duration,
    curve: [f32; 4],
}

impl Track {
    fn progress(&self, now: Instant) -> f32 {
        let elapsed = now.saturating_duration_since(self.started).as_secs_f32();
        (elapsed / self.duration.as_secs_f32().max(f32::EPSILON)).min(1.0)
    }

    fn value(&self, now: Instant) -> f32 {
        let progress = self.progress(now);
        self.from + (self.to - self.from) * cubic_bezier(self.curve, progress)
    }

    fn settled(&self, now: Instant) -> bool {
        self.progress(now) >= 1.0
    }
}

#[derive(Default)]
pub struct Motion {
    tracks: HashMap<Key, Track>,
}

impl Global for Motion {}

impl Motion {
    pub fn drive(&mut self, key: Key, on: bool, duration: Duration, curve: [f32; 4]) {
        let now = Instant::now();
        let to = if on { 1.0 } else { 0.0 };
        let from = self.tracks.get(&key).map_or(0.0, |track| track.value(now));
        if self.tracks.get(&key).is_some_and(|track| track.to == to) {
            return;
        }
        if !on && from <= 0.0 {
            self.tracks.remove(&key);
            return;
        }
        self.tracks.insert(
            key,
            Track {
                from,
                to,
                started: now,
                duration,
                curve,
            },
        );
    }

    pub fn value(&self, key: Key) -> f32 {
        self.tracks
            .get(&key)
            .map_or(0.0, |track| track.value(Instant::now()))
    }

    pub fn is_on(&self, key: Key) -> bool {
        self.tracks.get(&key).is_some_and(|track| track.to > 0.5)
    }

    fn settle(&mut self) -> bool {
        let now = Instant::now();
        self.tracks
            .retain(|_, track| !(track.to <= 0.0 && track.settled(now)));
        self.tracks.values().any(|track| !track.settled(now))
    }
}

pub fn hover(key: Key, on: bool, window: &mut Window, cx: &mut App) {
    cx.default_global::<Motion>().drive(key, on, HOVER, EASE);
    window.refresh();
}

pub fn drive(
    key: Key,
    on: bool,
    duration: Duration,
    curve: [f32; 4],
    window: &mut Window,
    cx: &mut App,
) {
    cx.default_global::<Motion>()
        .drive(key, on, duration, curve);
    window.refresh();
}

pub fn sync(key: Key, on: bool, duration: Duration, curve: [f32; 4], cx: &mut App) -> bool {
    let to = if on { 1.0 } else { 0.0 };
    let motion = cx.default_global::<Motion>();
    let current = motion.tracks.get(&key).map_or(0.0, |track| track.to);
    if current == to {
        return false;
    }
    motion.drive(key, on, duration, curve);
    true
}

pub fn release_hovers(cx: &mut App) {
    if let Some(motion) = cx.try_global::<Motion>()
        && motion
            .tracks
            .values()
            .any(|track| track.to > 0.0 && track.duration == HOVER)
    {
        cx.default_global::<Motion>()
            .tracks
            .retain(|_, track| track.duration != HOVER);
    }
}

pub fn restart(key: Key, duration: Duration, window: &mut Window, cx: &mut App) {
    let motion = cx.default_global::<Motion>();
    motion.tracks.insert(
        key,
        Track {
            from: 0.0,
            to: 1.0,
            started: Instant::now(),
            duration,
            curve: LINEAR,
        },
    );
    window.refresh();
}

pub fn stop(key: Key, window: &mut Window, cx: &mut App) {
    cx.default_global::<Motion>().tracks.remove(&key);
    window.refresh();
}

pub fn running(key: Key, cx: &App) -> Option<f32> {
    let motion = cx.try_global::<Motion>()?;
    let track = motion.tracks.get(&key)?;
    let now = Instant::now();
    (!track.settled(now)).then(|| track.progress(now))
}

pub fn value(key: Key, cx: &App) -> f32 {
    cx.try_global::<Motion>()
        .map_or(0.0, |motion| motion.value(key))
}

pub fn is_on(key: Key, cx: &App) -> bool {
    cx.try_global::<Motion>()
        .is_some_and(|motion| motion.is_on(key))
}

pub fn keep_alive(window: &mut Window, cx: &mut App) {
    if cx.default_global::<Motion>().settle() {
        window.request_animation_frame();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bezier_hits_endpoints() {
        assert!(cubic_bezier(EASE, 0.0).abs() < 1e-3);
        assert!((cubic_bezier(EASE, 1.0) - 1.0).abs() < 1e-3);
        assert!(cubic_bezier(EASE_SPRING, 0.6) > 1.0);
    }

    #[test]
    fn keyframes_interpolate() {
        let stops = [(0.0, 0.0), (0.5, 10.0), (1.0, 0.0)];
        assert!((keyframes(&stops, 0.5, LINEAR) - 10.0).abs() < 1e-3);
        assert!((keyframes(&stops, 0.25, LINEAR) - 5.0).abs() < 0.2);
        assert_eq!(keyframes(&stops, 2.0, LINEAR), 0.0);
    }

    #[test]
    fn keys_are_stable() {
        assert_eq!(Key::new("card", "a"), Key::new("card", "a"));
        assert_ne!(Key::new("card", "a"), Key::new("card", "b"));
        assert_eq!(Key::of("x"), Key::of("x"));
    }

    #[test]
    fn tracks_settle_and_drop() {
        let mut motion = Motion::default();
        let key = Key::of("k");
        motion.drive(key, true, Duration::ZERO, EASE);
        assert!(!motion.settle());
        assert!((motion.value(key) - 1.0).abs() < 1e-3);
        motion.drive(key, false, Duration::ZERO, EASE);
        motion.settle();
        assert!(motion.tracks.is_empty());
    }
}
