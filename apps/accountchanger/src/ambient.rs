use std::time::{Duration, Instant};

use gpui::{App, Global};

pub const FRAME: Duration = Duration::from_millis(16);
pub const CLOCK: Duration = Duration::from_secs(1);

pub struct Ambient {
    origin: Instant,
    live: bool,
}

impl Global for Ambient {}

impl Default for Ambient {
    fn default() -> Self {
        Self {
            origin: Instant::now(),
            live: false,
        }
    }
}

pub fn set_live(live: bool, cx: &mut App) {
    cx.default_global::<Ambient>().live = live;
}

pub fn is_live(cx: &App) -> bool {
    cx.try_global::<Ambient>()
        .is_some_and(|ambient| ambient.live)
}

pub fn cycle(period: Duration, cx: &App) -> Option<f32> {
    let ambient = cx.try_global::<Ambient>().filter(|ambient| ambient.live)?;
    let elapsed = ambient.origin.elapsed().as_secs_f32();
    Some((elapsed % period.as_secs_f32()) / period.as_secs_f32())
}
