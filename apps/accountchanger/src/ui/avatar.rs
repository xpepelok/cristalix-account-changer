use gpui::{
    App, Entity, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div, img,
    prelude::FluentBuilder, px,
};

use crate::state::Store;
use crate::theme::ThemeExt;

#[derive(IntoElement)]
pub struct Avatar {
    store: Entity<Store>,
    uuid: SharedString,
    size: f32,
    radius: f32,
}

impl Avatar {
    pub fn new(
        store: &Entity<Store>,
        uuid: impl Into<SharedString>,
        size: f32,
        radius: f32,
    ) -> Self {
        Self {
            store: store.clone(),
            uuid: uuid.into(),
            size,
            radius,
        }
    }
}

impl RenderOnce for Avatar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette();
        let pixels = (self.size * window.scale_factor()).round().max(8.0) as u32;
        let image = self
            .store
            .update(cx, |store, cx| store.head(&self.uuid, pixels, cx));
        div()
            .flex_none()
            .size(px(self.size))
            .rounded(px(self.radius))
            .overflow_hidden()
            .bg(p.surface_2)
            .when_some(image, |frame, image| {
                frame.child(img(image).size(px(self.size)).rounded(px(self.radius)))
            })
    }
}
