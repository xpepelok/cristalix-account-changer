use gpui::{Bounds, Pixels, Window};

#[derive(Clone)]
#[cfg_attr(not(windows), expect(dead_code))]
pub struct Prefill {
    pub username: String,
    pub password: String,
}

pub struct Signed {
    pub session: String,
    pub login: String,
    pub password: String,
}

pub use imp::LoginView;

#[cfg(windows)]
mod imp {
    use std::cell::{Cell, RefCell};
    use std::num::NonZeroIsize;
    use std::rc::Rc;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
    use std::time::Duration;

    use gpui::{Bounds, Pixels, Window};
    use raw_window_handle::{
        HandleError, HasWindowHandle, RawWindowHandle, Win32WindowHandle, WindowHandle,
    };
    use windows::Win32::Foundation::{HWND, POINT};
    use windows::Win32::Graphics::Gdi::ClientToScreen;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, DispatchMessageW, MSG, PM_REMOVE, PeekMessageW,
        SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOZORDER, SetWindowPos, ShowWindow,
        TranslateMessage, WS_CLIPCHILDREN, WS_EX_TOOLWINDOW, WS_POPUP,
    };
    use windows::core::w;
    use wry::dpi::{PhysicalPosition, PhysicalSize};
    use wry::{Rect, WebContext, WebViewBuilder};

    use super::{Prefill, Signed};

    const LOGIN_URL: &str = "https://cristalix.gg/cabinet/login";
    const ORIGIN: &str = "https://cristalix.gg";
    const CABINET: &str = "https://cristalix.gg/cabinet";
    const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
    const HIDDEN: &str = "#header,#boxheader,.header,.mobile-nav,.block-mobile-nav,footer,.userlang,.theme-switch,.box-toggle,.forgot-auth,#cookiePanel,.block-cookie,#sw-root,.grecaptcha-badge{display:none!important;}";
    const LAYOUT: &str = "html,body,#body{margin:0!important;padding:0!important;height:100%!important;overflow:hidden!important;}main,.block-auth{background:transparent!important;min-height:0!important;height:100%!important;display:flex!important;align-items:center!important;justify-content:center!important;padding:0!important;margin:0!important;}.box-auth{margin:0!important;box-shadow:none!important;background:transparent!important;border:none!important;}";
    const PUMP: Duration = Duration::from_millis(8);

    #[derive(Clone, Copy, PartialEq, Eq)]
    struct Area {
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    }

    enum Command {
        Place(Area),
        Close,
    }

    pub struct LoginView {
        commands: Sender<Command>,
        results: Receiver<Result<Signed, String>>,
        placed: Cell<Option<Bounds<Pixels>>>,
    }

    struct Parent(NonZeroIsize);

    impl HasWindowHandle for Parent {
        fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
            let raw = RawWindowHandle::Win32(Win32WindowHandle::new(self.0));
            Ok(unsafe { WindowHandle::borrow_raw(raw) })
        }
    }

    fn json_string(value: &str) -> String {
        let mut out = String::with_capacity(value.len() + 2);
        out.push('"');
        for ch in value.chars() {
            match ch {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\r' => {}
                _ => out.push(ch),
            }
        }
        out.push('"');
        out
    }

    fn init_script(prefill: Option<&Prefill>, background: &str) -> String {
        let css = format!("{HIDDEN}{LAYOUT}html,body,#body{{background:{background}!important;}}");
        let fill = prefill
            .map(|prefill| {
                format!(
                    "var u=document.getElementById('username-input');\
                     var p=document.getElementById('password-input');\
                     if(u){{u.value={user};u.dispatchEvent(new Event('input',{{bubbles:true}}));}}\
                     if(p&&{has_pass}){{p.value={pass};p.dispatchEvent(new Event('input',{{bubbles:true}}));}}",
                    user = json_string(&prefill.username),
                    pass = json_string(&prefill.password),
                    has_pass = !prefill.password.is_empty(),
                )
            })
            .unwrap_or_default();
        format!(
            "(function(){{var css={css};function report(){{var u=document.getElementById('username-input');\
             var p=document.getElementById('password-input');\
             if(u&&p&&window.ipc)window.ipc.postMessage(u.value+'\\n'+p.value);}}\
             function apply(){{var s=document.createElement('style');\
             s.textContent=css;(document.head||document.documentElement).appendChild(s);{fill}report();}}\
             ['input','change','submit','click','keydown'].forEach(function(e){{document.addEventListener(e,report,true);}});\
             if(document.readyState!=='loading')apply();else document.addEventListener('DOMContentLoaded',apply);}})();",
            css = json_string(&css),
        )
    }

    fn area(bounds: Bounds<Pixels>, scale: f32) -> Area {
        let scaled = |value: Pixels| (f32::from(value) * scale).round() as i32;
        Area {
            x: scaled(bounds.origin.x),
            y: scaled(bounds.origin.y),
            width: scaled(bounds.size.width).max(1),
            height: scaled(bounds.size.height).max(1),
        }
    }

    fn fill(area: Area) -> Rect {
        Rect {
            position: PhysicalPosition::new(0, 0).into(),
            size: PhysicalSize::new(area.width as u32, area.height as u32).into(),
        }
    }

    fn screen_of(owner: HWND, area: Area) -> (i32, i32) {
        let mut point = POINT {
            x: area.x,
            y: area.y,
        };
        unsafe {
            let _ = ClientToScreen(owner, &mut point);
        }
        (point.x, point.y)
    }

    fn popup(owner: HWND, area: Area) -> Option<HWND> {
        let (x, y) = screen_of(owner, area);
        unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW,
                w!("STATIC"),
                w!(""),
                WS_POPUP | WS_CLIPCHILDREN,
                x,
                y,
                area.width,
                area.height,
                Some(owner),
                None,
                None,
                None,
            )
            .ok()
        }
    }

    fn parent_of(window: &Window) -> Option<NonZeroIsize> {
        match HasWindowHandle::window_handle(window).ok()?.as_raw() {
            RawWindowHandle::Win32(handle) => Some(handle.hwnd),
            _ => None,
        }
    }

    fn pump() {
        let mut message = MSG::default();
        unsafe {
            while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }

    fn run(
        owner: NonZeroIsize,
        start: Area,
        prefill: Option<Prefill>,
        background: (u8, u8, u8),
        commands: Receiver<Command>,
        results: Sender<Result<Signed, String>>,
    ) {
        let owner = HWND(owner.get() as *mut core::ffi::c_void);
        let Some(host) = popup(owner, start) else {
            results
                .send(Err("не удалось открыть окно входа".to_owned()))
                .ok();
            return;
        };
        let Some(handle) = NonZeroIsize::new(host.0 as isize) else {
            return;
        };
        let logged_in = Arc::new(AtomicBool::new(false));
        let signal = logged_in.clone();
        let typed: Rc<RefCell<(String, String)>> = Rc::default();
        let sink = typed.clone();
        let mut context = WebContext::new(Some(super::profile_dir()));
        let css_background = format!(
            "#{:02x}{:02x}{:02x}",
            background.0, background.1, background.2
        );
        let built = WebViewBuilder::new_with_web_context(&mut context)
            .with_url(LOGIN_URL)
            .with_user_agent(UA)
            .with_background_color((background.0, background.1, background.2, 255))
            .with_bounds(fill(start))
            .with_initialization_script(init_script(prefill.as_ref(), &css_background))
            .with_ipc_handler(move |request| {
                if !request.uri().to_string().starts_with(ORIGIN) {
                    return;
                }
                if let Some((login, password)) = request.body().split_once('\n') {
                    *sink.borrow_mut() = (login.trim().to_owned(), password.to_owned());
                }
            })
            .with_navigation_handler(move |url: String| {
                let done = url.starts_with(CABINET) && !url.contains("/cabinet/login");
                if done {
                    signal.store(true, Ordering::Relaxed);
                }
                !done
            })
            .build_as_child(&Parent(handle));
        let webview = match built {
            Ok(webview) => webview,
            Err(err) => {
                results.send(Err(err.to_string())).ok();
                unsafe {
                    let _ = DestroyWindow(host);
                }
                return;
            }
        };
        unsafe {
            let _ = ShowWindow(host, SW_SHOWNOACTIVATE);
        }
        let mut area = start;
        let mut placed = (screen_of(owner, area), area);
        let mut reported = false;
        'pump: loop {
            pump();
            loop {
                match commands.try_recv() {
                    Ok(Command::Place(next)) => area = next,
                    Ok(Command::Close) | Err(TryRecvError::Disconnected) => break 'pump,
                    Err(TryRecvError::Empty) => break,
                }
            }
            let now = (screen_of(owner, area), area);
            if now != placed {
                placed = now;
                let ((x, y), area) = now;
                unsafe {
                    let _ = SetWindowPos(
                        host,
                        None,
                        x,
                        y,
                        area.width,
                        area.height,
                        SWP_NOACTIVATE | SWP_NOZORDER,
                    );
                }
                webview.set_bounds(fill(area)).ok();
            }
            if !reported && logged_in.load(Ordering::Relaxed) {
                reported = true;
                webview.set_visible(false).ok();
                let session = webview.cookies_for_url(ORIGIN).ok().and_then(|cookies| {
                    cookies
                        .iter()
                        .find(|cookie| cookie.name() == "PHPSESSID")
                        .map(|cookie| cookie.value().to_owned())
                });
                results
                    .send(match session {
                        Some(session) if !session.is_empty() => {
                            let (login, password) = typed.take();
                            Ok(Signed {
                                session,
                                login,
                                password,
                            })
                        }
                        _ => Err("не удалось получить сессию".to_owned()),
                    })
                    .ok();
            }
            std::thread::sleep(PUMP);
        }
        drop(webview);
        unsafe {
            let _ = DestroyWindow(host);
        }
    }

    impl LoginView {
        pub fn create(
            window: &Window,
            bounds: Bounds<Pixels>,
            prefill: Option<&Prefill>,
            background: (u8, u8, u8),
        ) -> Result<Self, String> {
            let owner = parent_of(window).ok_or("нет окна для входа")?;
            let (commands, inbox) = mpsc::channel();
            let (outbox, results) = mpsc::channel();
            let prefill = prefill.cloned();
            let start = area(bounds, window.scale_factor());
            std::thread::Builder::new()
                .name("cabinet-login".into())
                .spawn(move || run(owner, start, prefill, background, inbox, outbox))
                .map_err(|err| err.to_string())?;
            Ok(Self {
                commands,
                results,
                placed: Cell::new(Some(bounds)),
            })
        }

        pub fn place(&self, bounds: Bounds<Pixels>, scale: f32) {
            if self.placed.get() == Some(bounds) {
                return;
            }
            self.placed.set(Some(bounds));
            self.commands.send(Command::Place(area(bounds, scale))).ok();
        }

        pub fn session(&self) -> Option<Result<Signed, String>> {
            self.results.try_recv().ok()
        }
    }

    impl Drop for LoginView {
        fn drop(&mut self) {
            self.commands.send(Command::Close).ok();
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use gpui::{Bounds, Pixels, Window};

    use super::{Prefill, Signed};

    pub struct LoginView;

    impl LoginView {
        pub fn create(
            _window: &Window,
            _bounds: Bounds<Pixels>,
            _prefill: Option<&Prefill>,
            _background: (u8, u8, u8),
        ) -> Result<Self, String> {
            Err("вход в кабинет пока доступен только на Windows".to_owned())
        }

        pub fn place(&self, _bounds: Bounds<Pixels>, _scale: f32) {}

        pub fn session(&self) -> Option<Result<Signed, String>> {
            None
        }
    }
}

#[cfg(windows)]
fn profile_dir() -> std::path::PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("AccountChanger").join("cabinet-webview")
}

pub fn create(
    window: &Window,
    bounds: Bounds<Pixels>,
    prefill: Option<&Prefill>,
    background: (u8, u8, u8),
) -> Result<LoginView, String> {
    LoginView::create(window, bounds, prefill, background)
}
