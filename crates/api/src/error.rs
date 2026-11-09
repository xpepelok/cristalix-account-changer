use std::borrow::Cow;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct ApiError {
    message: Cow<'static, str>,
}

impl ApiError {
    pub const fn fixed(message: &'static str) -> Self {
        Self {
            message: Cow::Borrowed(message),
        }
    }

    pub fn new(message: impl Into<Cow<'static, str>>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl From<&'static str> for ApiError {
    fn from(message: &'static str) -> Self {
        Self::fixed(message)
    }
}

impl From<String> for ApiError {
    fn from(message: String) -> Self {
        Self::new(message)
    }
}

pub type ApiResult<T> = Result<T, ApiError>;

pub mod msg {
    pub const BAD_REQUEST: &str = "bad request";
    pub const GENERIC: &str = "Ошибка";
    pub const NO_TOKEN: &str = "нет сохранённого токена для этого аккаунта";
    pub const NO_SAVED_TOKEN: &str = "нет сохранённого токена";
    pub const NO_NAME: &str = "no name";
    pub const PROFILE_NAME_REQUIRED: &str = "введите название профиля";
    pub const PROFILE_NAME_RESERVED: &str = "это имя занято встроенным профилем";
    pub const PROFILE_NO_CLIENT: &str = "не найден клиент для чтения настроек";
    pub const PROFILE_NOTHING_TO_SAVE: &str = "в клиенте нет файлов настроек для сохранения";
    pub const PROFILE_BUILTIN_READONLY: &str = "встроенный профиль нельзя редактировать";
    pub const PROFILE_BUILTIN_UNDELETABLE: &str = "встроенный профиль нельзя удалить";
    pub const NO_UPDATE: &str = "нет доступного обновления";
    pub const CABINET_SESSION: &str = "сессия истекла, войди в кабинет заново";
    pub const GROUP_NOT_FOUND: &str = "группа не найдена";
    pub const GROUP_EMPTY: &str = "в группе нет аккаунтов";
    pub const GROUP_NAME_EMPTY: &str = "пустое название";
    pub const CUSTOM_LAUNCHER_REQUIRED: &str = "не указан путь к лаунчеру";
    pub const LAUNCHER_WINDOWS_ONLY: &str =
        "этот лаунчер - Windows-программа, на Linux используй jar или свой";
    pub const LAUNCHER_UNKNOWN: &str = "неизвестный лаунчер";
    pub const NO_UI_AUTOMATION: &str = "импорт по логину/паролю и обновление токенов недоступны на Linux: войди в аккаунт через лаунчер, токен подхватится автоматически";
    pub const IMPORT_RUNNING: &str = "импорт уже идёт";
    pub const IMPORT_ALL_DUPLICATES: &str = "в списке только дубликаты";
    pub const IMPORT_EMPTY: &str = "список пуст";
    pub const REFRESH_RUNNING: &str = "Процесс уже идёт";
    pub const REFRESH_NOTHING: &str = "Нет токенов, истекающих в ближайшую неделю";
    pub const CANCELED: &str = "отменено";
    pub const WRONG_CREDENTIALS: &str = "неправильный логин или пароль";
    pub const CREDENTIALS_REQUIRED: &str = "укажите логин и пароль";
    pub const AUTH_LOGIN_UNKNOWN: &str = "Неизвестная ошибка: auth-login";
    pub const LAUNCHER_WINDOW_MISSING: &str = "окно лаунчера не появилось";
    pub const LOGIN_FORM_MISSING: &str = "окно входа не найдено (форма авторизации не появилась)";
    pub const LAUNCHER_FOCUS_ADMIN: &str = "не удалось вывести окно лаунчера на передний план — возможно, он запущен от имени администратора. Запусти AccountChanger тоже от администратора и повтори.";
    pub const LAUNCHER_INPUT_ADMIN: &str = "не удалось ввести данные — похоже, лаунчер запущен от имени администратора, а AccountChanger нет. Запусти AccountChanger тоже от администратора и повтори.";
    pub const LAUNCHER_FOCUS_FAILED: &str = "не удалось сфокусироваться на окне лаунчера";
    pub const CREDENTIALS_INPUT_FAILED: &str = "ошибка ввода логина/пароля";
    pub const LOGIN_TIMEOUT: &str =
        "вход не удался или превышено время ожидания (проверь логин/пароль, возможна капча/2FA)";
    pub const TOKEN_EXPIRED_NEEDS_PASSWORD: &str =
        "токен истёк - лаунчер просит пароль, войди вручную или импортируй по логину/паролю";
    pub const IMPORT_SKIP_SAVED: &str = "уже сохранён";
    pub const IMPORT_QUEUED: &str = "в очереди";
    pub const REFRESH_WORKING: &str = "обновление…";
    pub const REFRESH_TOO_EARLY: &str = "ещё рано - токен в силе";
    pub const GROUP_SKIP_NO_TOKEN: &str = "нет сохранённого токена";
    pub const GROUP_SKIP_RUNNING: &str = "уже запущен";
    pub const GROUP_SKIP_APPLY_FAILED: &str = "не удалось применить токен";
    pub const GROUP_SKIP_LAUNCHER_FAILED: &str = "лаунчер не запустился";

    pub fn launcher_prepare_failed(cause: &str) -> String {
        format!("не удалось подготовить лаунчер: {cause}")
    }

    pub fn launcher_start_failed(cause: &str) -> String {
        format!("не удалось запустить лаунчер: {cause}")
    }

    pub fn update_download_failed(status: u16) -> String {
        format!("не удалось скачать обновление (код {status})")
    }

    pub fn import_nothing(skipped: usize, duplicates: usize) -> String {
        match (skipped, duplicates) {
            (0, 0) => IMPORT_EMPTY.to_owned(),
            (0, _) => IMPORT_ALL_DUPLICATES.to_owned(),
            (s, 0) => format!("все аккаунты уже сохранены ({s})"),
            (s, d) => format!("нечего импортировать: уже сохранено {s}, дублей {d}"),
        }
    }
}
