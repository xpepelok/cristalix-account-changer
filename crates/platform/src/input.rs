use std::mem::size_of;
use std::thread;
use std::time::Duration;

use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP,
    KEYEVENTF_UNICODE, SendInput, VIRTUAL_KEY, VK_RETURN, VK_TAB,
};

const TYPE_DELAY: Duration = Duration::from_millis(14);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Tab,
    Enter,
}

pub fn type_text(text: &str) {
    for ch in text.chars() {
        if let Ok(unit) = u16::try_from(u32::from(ch)) {
            stroke(VIRTUAL_KEY(0), unit, KEYEVENTF_UNICODE);
        }
        thread::sleep(TYPE_DELAY);
    }
}

pub fn press_key(key: Key) {
    let vk = match key {
        Key::Tab => VK_TAB,
        Key::Enter => VK_RETURN,
    };
    stroke(vk, 0, KEYBD_EVENT_FLAGS(0));
}

fn stroke(vk: VIRTUAL_KEY, scan: u16, flags: KEYBD_EVENT_FLAGS) {
    let inputs = [
        keyboard(vk, scan, flags),
        keyboard(vk, scan, flags | KEYEVENTF_KEYUP),
    ];
    unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
}

fn keyboard(vk: VIRTUAL_KEY, scan: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}
