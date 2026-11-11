mod avatar;
mod button;
mod controls;
pub mod fit;
mod input;
mod menu;
mod modal;
mod scroll;
mod text;
mod tooltip;

pub use avatar::Avatar;
pub use button::{Button, ButtonStyle, IconButton, IconStyle};
pub use controls::{Badge, BadgeTone, Checkbox, Chip, Segmented, Slider, Spinner, Toggle};
pub use input::{InputEvent, TextInput, bind_keys as bind_input_keys};
pub use menu::{MenuItem, drop_in, menu_item, menu_separator, menu_surface, pop_in};
pub use modal::{ModalFrame, modal_head};
pub use scroll::{ScrollArea, scrollbar};
pub use text::{spaced, spaced_chars};
pub use tooltip::{Tooltips, tooltip_overlay, with_tooltip};

use gpui::{App, ClickEvent, Window};

pub type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;
