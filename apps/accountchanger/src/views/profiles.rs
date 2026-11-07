use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use ac_api::{PROFILE_FILES, PROFILE_NAME_MAX_CHARS, ProfileFiles};
use gpui::{
    AnyElement, AppContext, Context, ElementId, Entity, ExternalPaths, FontWeight,
    InteractiveElement, IntoElement, ParentElement, ScrollHandle, StatefulInteractiveElement,
    Styled, Window, div, prelude::FluentBuilder, px, relative,
};

use crate::assets::{Icon, icon};
use crate::motion::{self, Key};
use crate::sound::{self, Sound};
use crate::theme::{ThemeExt, mix, rem, rgba};
use crate::ui::{
    Button, ButtonStyle, IconButton, IconStyle, InputEvent, ModalFrame, ScrollArea, TextInput,
    modal_head, spaced,
};
use crate::views::shell::{Overlay, Shell};
use crate::views::widgets::{drop_zone, field, text_field};

const KV_FILES: [&str; 2] = ["options.txt", "optionsof.txt"];
const JSON_FILES: [&str; 2] = ["binds.json", "voicechat.json"];

enum KvLine {
    Pair {
        key: String,
        value: Entity<TextInput>,
    },
    Raw(String),
}

struct Editor {
    name: String,
    builtin: bool,
    kv: BTreeMap<&'static str, Vec<KvLine>>,
    json: BTreeMap<&'static str, Entity<TextInput>>,
    invalid: BTreeSet<&'static str>,
}

pub struct ProfilesView {
    editor: Option<Editor>,
    list_scroll: ScrollHandle,
    editor_scroll: ScrollHandle,
}

pub struct ProfileImportView {
    name: Entity<TextInput>,
    files: BTreeMap<String, String>,
}

pub fn pretty_json(text: &str) -> String {
    if text.trim().is_empty() {
        return text.to_owned();
    }
    serde_json::from_str::<serde_json::Value>(text)
        .ok()
        .and_then(|value| serde_json::to_string_pretty(&value).ok())
        .unwrap_or_else(|| text.to_owned())
}

pub fn json_valid(text: &str) -> bool {
    text.trim().is_empty() || serde_json::from_str::<serde_json::Value>(text).is_ok()
}

impl Editor {
    fn collect(&self, cx: &gpui::App) -> ProfileFiles {
        let mut files = ProfileFiles::new();
        for (file, lines) in &self.kv {
            let text = lines
                .iter()
                .map(|line| match line {
                    KvLine::Pair { key, value } => format!("{key}:{}", value.read(cx).text()),
                    KvLine::Raw(raw) => raw.clone(),
                })
                .collect::<Vec<_>>()
                .join("\n");
            files.insert((*file).to_owned(), text);
        }
        for (file, input) in &self.json {
            files.insert((*file).to_owned(), input.read(cx).text().to_owned());
        }
        files
    }
}

impl ProfilesView {
    pub fn render(
        &mut self,
        shell: &mut Shell,
        window: &mut Window,
        cx: &mut Context<Shell>,
    ) -> AnyElement {
        let p = cx.palette();
        let profiles = shell.store.read(cx).profiles.clone();
        let editing = self.editor.as_ref().map(|e| e.name.clone());
        let items: Vec<AnyElement> = profiles
            .profiles
            .iter()
            .map(|name| {
                let active = editing.as_deref() == Some(name.as_str());
                let builtin = profiles.is_builtin(name);
                let key = Key::new("profile-item", name);
                let hover = motion::value(key, cx);
                let target = name.clone();
                div()
                    .id(ElementId::Name(format!("profile-{name}").into()))
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(px(8.0))
                    .px(px(11.0))
                    .py(px(9.0))
                    .rounded(px(9.0))
                    .border_1()
                    .border_color(if active {
                        rgba(0x3b82e8, 0.35)
                    } else {
                        rgba(0x000000, 0.0)
                    })
                    .bg(if active {
                        rgba(0x3b82e8, 0.12)
                    } else {
                        mix(rgba(0x000000, 0.0), p.surface_2, hover)
                    })
                    .text_color(if active {
                        p.text
                    } else {
                        mix(p.muted, p.text, hover)
                    })
                    .text_size(rem(0.84))
                    .font_weight(FontWeight::SEMIBOLD)
                    .cursor_pointer()
                    .on_hover(move |hovered, window, cx| motion::hover(key, *hovered, window, cx))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        sound::play(Sound::Tap);
                        this.edit_profile(target.clone(), window, cx);
                    }))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .child(name.clone()),
                    )
                    .when(builtin, |row| {
                        row.child(
                            div()
                                .ml_auto()
                                .px(px(7.0))
                                .py(px(2.0))
                                .rounded(px(8.0))
                                .bg(p.blue)
                                .opacity(0.85)
                                .text_size(px(10.0))
                                .text_color(p.white)
                                .child("встроенный"),
                        )
                    })
                    .into_any_element()
            })
            .collect();
        let list_body = if items.is_empty() {
            div()
                .py(px(18.0))
                .px(px(8.0))
                .text_center()
                .text_size(rem(0.85))
                .text_color(p.faint)
                .child("Профилей пока нет")
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_col()
                .gap(px(4.0))
                .pr(px(8.0))
                .children(items)
                .into_any_element()
        };
        let side =
            div()
                .flex_none()
                .w(px(280.0))
                .flex()
                .flex_col()
                .gap(px(8.0))
                .p(px(14.0))
                .border_r_1()
                .border_color(p.line)
                .overflow_hidden()
                .child(
                    div()
                        .flex()
                        .gap(px(8.0))
                        .child(
                            Button::new(
                                "profile-save-current",
                                "＋ Из текущих настроек",
                                ButtonStyle::Primary,
                            )
                            .grow()
                            .on_click(cx.listener(
                                |this, _, window, cx| this.save_current_profile(window, cx),
                            )),
                        )
                        .child(
                            IconButton::new(
                                "profile-import",
                                Key::of("profile-import"),
                                IconStyle::Square,
                            )
                            .icon(Icon::Download, 18.0)
                            .size(40.0, 41.0)
                            .tooltip("Импорт профиля из файлов")
                            .on_click(cx.listener(
                                |this, _, window, cx| this.open_profile_import(window, cx),
                            )),
                        ),
                )
                .child(ScrollArea::new("profiles-list", &self.list_scroll).child(list_body));

        let editor = match &self.editor {
            None => div()
                .flex_1()
                .min_w_0()
                .px(px(20.0))
                .py(px(50.0))
                .text_center()
                .text_size(rem(0.85))
                .line_height(relative(1.6))
                .text_color(p.faint)
                .child("Выбери профиль слева, чтобы отредактировать его параметры, либо создай новый из текущих настроек игры.")
                .into_any_element(),
            Some(editor) => {
                let mut body = div().flex().flex_col().child(
                    div()
                        .mb(px(12.0))
                        .overflow_hidden()
                        .text_size(rem(1.0))
                        .font_weight(FontWeight::BOLD)
                        .child(editor.name.clone()),
                );
                let title = |file: &str| {
                    div()
                        .mb(px(7.0))
                        .text_size(rem(0.66))
                        .font_weight(FontWeight::BOLD)
                        .text_color(p.blue)
                        .child(spaced(&file.to_uppercase(), 0.4))
                };
                for file in KV_FILES {
                    let lines = editor.kv.get(file).map(Vec::as_slice).unwrap_or_default();
                    let rows: Vec<AnyElement> = lines
                        .iter()
                        .filter_map(|line| match line {
                            KvLine::Pair { key, value } => {
                                let focused = value.read(cx).is_focused(window);
                                Some(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(8.0))
                                        .child(
                                            div()
                                                .flex_none()
                                                .w(relative(0.42))
                                                .overflow_hidden()
                                                .text_size(rem(0.74))
                                                .text_color(p.muted)
                                                .child(key.clone()),
                                        )
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w_0()
                                                .flex()
                                                .px(px(9.0))
                                                .py(px(5.0))
                                                .rounded(px(7.0))
                                                .border_1()
                                                .border_color(if focused { p.blue } else { p.line })
                                                .bg(p.surface_2)
                                                .text_size(rem(0.76))
                                                .when(editor.builtin, |field| field.opacity(0.6))
                                                .child(value.clone()),
                                        )
                                        .into_any_element(),
                                )
                            }
                            KvLine::Raw(_) => None,
                        })
                        .collect();
                    let empty = rows.is_empty();
                    body = body.child(
                        div()
                            .mb(px(16.0))
                            .child(title(file))
                            .child(div().flex().flex_col().gap(px(5.0)).children(rows))
                            .when(empty, |block| {
                                block.child(div().p(px(8.0)).text_size(rem(0.85)).text_color(p.faint).child("файл пуст"))
                            }),
                    );
                }
                for file in JSON_FILES {
                    let Some(input) = editor.json.get(file) else {
                        continue;
                    };
                    let focused = input.read(cx).is_focused(window);
                    let invalid = editor.invalid.contains(file);
                    body = body.child(
                        div().mb(px(16.0)).child(title(file)).child(
                            div()
                                .min_h(px(90.0))
                                .px(px(11.0))
                                .py(px(9.0))
                                .rounded(px(9.0))
                                .border_1()
                                .border_color(if invalid {
                                    p.red
                                } else if focused {
                                    p.blue
                                } else {
                                    p.line
                                })
                                .bg(p.surface_2)
                                .text_size(rem(0.76))
                                .line_height(relative(1.5))
                                .child(input.clone()),
                        ),
                    );
                }
                let actions = if editor.builtin {
                    div()
                        .flex_none()
                        .flex()
                        .items_center()
                        .px(px(18.0))
                        .py(px(12.0))
                        .border_t_1()
                        .border_color(p.line)
                        .bg(p.surface)
                        .text_size(rem(0.85))
                        .text_color(p.faint)
                        .child("Встроенный профиль — только для чтения")
                } else {
                    let name = editor.name.clone();
                    let delete = editor.name.clone();
                    div()
                        .flex_none()
                        .flex()
                        .gap(px(8.0))
                        .px(px(18.0))
                        .py(px(12.0))
                        .border_t_1()
                        .border_color(p.line)
                        .bg(p.surface)
                        .child(
                            Button::new("editor-save", "Сохранить профиль", ButtonStyle::Primary)
                                .on_click(cx.listener(move |this, _, _, cx| this.save_profile(name.clone(), cx))),
                        )
                        .child(div().flex_1())
                        .child(
                            Button::new("editor-delete", "Удалить", ButtonStyle::DangerGhost)
                                .on_click(cx.listener(move |this, _, window, cx| this.delete_profile(delete.clone(), window, cx))),
                        )
                };
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .child(ScrollArea::new("profile-editor", &self.editor_scroll).child(div().px(px(18.0)).py(px(16.0)).child(body)))
                    .child(actions)
                    .into_any_element()
            }
        };
        let height = f32::from(window.viewport_size().height) * 0.82;
        ModalFrame::new("profiles-overlay", 860.0, Shell::closer::<ProfilesView>(cx))
            .child(
                div()
                    .h(px(height))
                    .flex()
                    .flex_col()
                    .child(modal_head(
                        "Профили настроек",
                        Some("Наборы binds / options / optionsof / voicechat, которые можно применять к аккаунтам".into()),
                        cx,
                    ))
                    .child(div().flex().flex_1().min_h_0().child(side).child(editor)),
            )
            .into_any_element()
    }
}

impl ProfileImportView {
    pub fn render(
        &mut self,
        _shell: &mut Shell,
        window: &mut Window,
        cx: &mut Context<Shell>,
    ) -> AnyElement {
        let p = cx.palette();
        let name = self.name.read(cx).text().trim().to_owned();
        let ready = !name.is_empty() && !self.files.is_empty();
        let chips = self
            .files
            .keys()
            .cloned()
            .map(|file| {
                let key = Key::new("import-chip", &file);
                let hover = motion::value(key, cx);
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .px(px(11.0))
                    .py(px(7.0))
                    .rounded(px(9.0))
                    .border_1()
                    .border_color(p.line)
                    .bg(p.surface_2)
                    .text_size(rem(0.8))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(
                        div()
                            .text_color(p.green)
                            .font_weight(FontWeight::BOLD)
                            .child("✓"),
                    )
                    .child(file.clone())
                    .child(
                        div()
                            .id(ElementId::Name(format!("chip-rm-{file}").into()))
                            .ml_auto()
                            .px(px(4.0))
                            .cursor_pointer()
                            .on_hover(move |hovered, window, cx| {
                                motion::hover(key, *hovered, window, cx)
                            })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if let Some(view) = this.overlay::<ProfileImportView>() {
                                    view.files.remove(&file);
                                }
                                cx.notify();
                            }))
                            .child(icon(Icon::XMark, px(11.0), mix(p.faint, p.red, hover))),
                    )
            })
            .collect::<Vec<_>>();
        ModalFrame::new(
            "import-overlay",
            470.0,
            Shell::closer::<ProfileImportView>(cx),
        )
        .child(modal_head(
            "Импорт настроек",
            Some("binds.json / options.txt / optionsof.txt / voicechat.json".into()),
            cx,
        ))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(14.0))
                .pt(px(18.0))
                .px(px(20.0))
                .pb(px(20.0))
                .child(field(
                    "Название профиля",
                    text_field(&self.name, window, cx),
                    cx,
                ))
                .child(
                    drop_zone(
                        "import-drop",
                        "Перетащите файлы с настройками или нажмите сюда для выбора",
                        false,
                        cx,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        sound::play(Sound::Tap);
                        this.pick_profile_files(cx);
                    }))
                    .on_drop(cx.listener(
                        |this, paths: &ExternalPaths, _, cx| {
                            this.ingest_profile_files(paths.paths().to_vec(), cx)
                        },
                    )),
                )
                .child(div().flex().flex_col().gap(px(6.0)).children(chips))
                .child(
                    Button::new("import-apply", "Импортировать", ButtonStyle::Primary)
                        .full()
                        .disabled(!ready)
                        .on_click(
                            cx.listener(|this, _, window, cx| {
                                this.apply_profile_import(window, cx)
                            }),
                        ),
                ),
        )
        .into_any_element()
    }
}

impl Shell {
    pub(crate) fn open_profiles(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.store.update(cx, |store, cx| store.reload_profiles(cx));
        self.push_overlay(
            Overlay::Profiles(ProfilesView {
                editor: None,
                list_scroll: ScrollHandle::new(),
                editor_scroll: ScrollHandle::new(),
            }),
            cx,
        );
    }

    fn edit_profile(&mut self, name: String, window: &mut Window, cx: &mut Context<Self>) {
        let builtin = self.store.read(cx).profiles.is_builtin(&name);
        let future = self.store.read(cx).backend.profile_content(name.clone());
        cx.spawn_in(window, async move |this, cx| {
            let content = future.await.unwrap_or_default();
            this.update_in(cx, |shell, window, cx| {
                let mut kv = BTreeMap::new();
                for file in KV_FILES {
                    let text = content.get(file).cloned().unwrap_or_default();
                    let lines = text
                        .split('\n')
                        .map(|line| {
                            let line = line.trim_end_matches('\r');
                            match line.find(':').filter(|index| *index > 0) {
                                Some(index) => {
                                    let value = line[index + 1..].to_owned();
                                    let input = cx.new(|cx| {
                                        let mut input = TextInput::new("", window, cx);
                                        input.set_text(value, cx);
                                        input
                                    });
                                    KvLine::Pair {
                                        key: line[..index].to_owned(),
                                        value: input,
                                    }
                                }
                                None => KvLine::Raw(line.to_owned()),
                            }
                        })
                        .collect();
                    kv.insert(file, lines);
                }
                let mut json = BTreeMap::new();
                for file in JSON_FILES {
                    let text =
                        pretty_json(content.get(file).map(String::as_str).unwrap_or_default());
                    let input = cx.new(|cx| {
                        let mut input = TextInput::new("", window, cx).multiline();
                        input.set_text(text, cx);
                        input
                    });
                    json.insert(file, input);
                }
                if let Some(view) = shell.overlay::<ProfilesView>() {
                    view.editor = Some(Editor {
                        name,
                        builtin,
                        kv,
                        json,
                        invalid: BTreeSet::new(),
                    });
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn save_current_profile(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.prompt(
            "Название нового профиля",
            "",
            PROFILE_NAME_MAX_CHARS,
            |this, name, window, cx| {
                let future = this.store.read(cx).backend.save_profile(name, None);
                cx.spawn_in(window, async move |this, cx| {
                    let result = future.await;
                    this.update_in(cx, |shell, window, cx| match result {
                        Ok(saved) => {
                            shell.toast(format!("Сохранено файлов: {}", saved.copied), false, cx);
                            shell
                                .store
                                .update(cx, |store, cx| store.reload_profiles(cx));
                            shell.edit_profile(saved.name, window, cx);
                        }
                        Err(error) => shell.toast(error.message().to_owned(), true, cx),
                    })
                    .ok();
                })
                .detach();
            },
            window,
            cx,
        );
    }

    fn save_profile(&mut self, name: String, cx: &mut Context<Self>) {
        let Some(view) = self.overlay::<ProfilesView>() else {
            return;
        };
        let Some(editor) = view.editor.as_mut() else {
            return;
        };
        editor.invalid.clear();
        let files = editor.collect(cx);
        for file in JSON_FILES {
            if !json_valid(files.get(file).map(String::as_str).unwrap_or_default()) {
                editor.invalid.insert(file);
                self.toast(format!("Некорректный JSON в {file}"), true, cx);
                cx.notify();
                return;
            }
        }
        self.store.update(cx, |store, cx| {
            let future = store.backend.update_profile(name, files);
            store.act(cx, future, "Профиль сохранён");
        });
    }

    fn delete_profile(&mut self, name: String, window: &mut Window, cx: &mut Context<Self>) {
        self.confirm(
            "Удалить профиль?",
            format!("Вы уверены, что хотите удалить профиль «{name}»?"),
            "Да, удалить",
            move |this, _, cx| {
                if let Some(view) = this.overlay::<ProfilesView>() {
                    view.editor = None;
                }
                this.store.update(cx, |store, cx| {
                    let future = store.backend.delete_profile(name);
                    store.fetch(cx, future, |store, (), cx| {
                        store.toast("Профиль удалён", false, cx);
                        store.reload_profiles(cx);
                        store.reload_accounts(cx);
                    });
                });
            },
            window,
            cx,
        );
    }

    fn open_profile_import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = cx.new(|cx| {
            TextInput::new("Например: Anarchy-настройки", window, cx)
                .max_chars(PROFILE_NAME_MAX_CHARS)
        });
        cx.subscribe(&name, |_, _, event, cx| {
            if *event == InputEvent::Changed {
                cx.notify();
            }
        })
        .detach();
        name.read(cx).focus(window);
        self.push_overlay(
            Overlay::ProfileImport(ProfileImportView {
                name,
                files: BTreeMap::new(),
            }),
            cx,
        );
    }

    fn pick_profile_files(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let Some(files) = rfd::AsyncFileDialog::new()
                .add_filter("Настройки", &["txt", "json"])
                .pick_files()
                .await
            else {
                return;
            };
            let paths = files.iter().map(|file| file.path().to_path_buf()).collect();
            this.update(cx, |shell, cx| shell.ingest_profile_files(paths, cx))
                .ok();
        })
        .detach();
    }

    fn ingest_profile_files(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        let mut skipped = Vec::new();
        let mut accepted = Vec::new();
        for path in paths {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let key = name.to_lowercase();
            if !PROFILE_FILES.contains(&key.as_str()) {
                skipped.push(name);
                continue;
            }
            if let Ok(text) = std::fs::read_to_string(&path) {
                accepted.push((key, text));
            }
        }
        if let Some(view) = self.overlay::<ProfileImportView>() {
            view.files.extend(accepted);
        }
        if !skipped.is_empty() {
            self.toast(
                format!("Пропущено (не файлы настроек): {}", skipped.join(", ")),
                true,
                cx,
            );
        }
        cx.notify();
    }

    fn apply_profile_import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(view) = self.overlay::<ProfileImportView>() else {
            return;
        };
        let name = view.name.read(cx).text().trim().to_owned();
        let files: ProfileFiles = view.files.clone().into_iter().collect();
        if name.is_empty() || files.is_empty() {
            return;
        }
        let future = self
            .store
            .read(cx)
            .backend
            .update_profile(name.clone(), files);
        cx.spawn_in(window, async move |this, cx| {
            let result = future.await;
            this.update_in(cx, |shell, window, cx| match result {
                Ok(()) => {
                    shell.toast(format!("Профиль импортирован: {name}"), false, cx);
                    shell.close::<ProfileImportView>(window, cx);
                    shell
                        .store
                        .update(cx, |store, cx| store.reload_profiles(cx));
                    shell.edit_profile(name, window, cx);
                }
                Err(error) => shell.toast(error.message().to_owned(), true, cx),
            })
            .ok();
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_helpers() {
        assert!(json_valid(""));
        assert!(json_valid("{\"a\":1}"));
        assert!(!json_valid("{"));
        assert_eq!(pretty_json("{\"a\":1}"), "{\n  \"a\": 1\n}");
        assert_eq!(pretty_json("nope"), "nope");
    }
}
