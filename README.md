# AccountChanger

[![Скачиваний](https://img.shields.io/github/downloads/xpepelok/cristalix-account-changer/total?color=1fa344&label=%D1%81%D0%BA%D0%B0%D1%87%D0%B0%D0%BD%D0%BE&labelColor=64748b&logo=github&logoColor=white)](https://github.com/xpepelok/cristalix-account-changer/releases)
[![Версия](https://img.shields.io/github/v/release/xpepelok/cristalix-account-changer?color=3b82e8&label=%D0%B2%D0%B5%D1%80%D1%81%D0%B8%D1%8F&labelColor=64748b&logo=github&logoColor=white)](https://github.com/xpepelok/cristalix-account-changer/releases/latest)
![Платформа](https://img.shields.io/badge/%D0%BF%D0%BB%D0%B0%D1%82%D1%84%D0%BE%D1%80%D0%BC%D0%B0-Windows%20%7C%20Linux-3b82e8?labelColor=64748b&logo=linux&logoColor=white)
![Просмотры](https://visitor-badge.laobi.icu/badge?page_id=xpepelok.cristalix-account-changer&left_text=%D0%BF%D1%80%D0%BE%D1%81%D0%BC%D0%BE%D1%82%D1%80%D1%8B&left_color=64748b&right_color=3b82e8)
![Сейчас онлайн](https://img.shields.io/badge/dynamic/json?url=https://stats.xpepelok.me/stats&query=%24.online&label=%D1%81%D0%B5%D0%B9%D1%87%D0%B0%D1%81%20%D0%BE%D0%BD%D0%BB%D0%B0%D0%B9%D0%BD&color=3b82e8&labelColor=64748b)
![Всего пользователей](https://img.shields.io/endpoint?url=https://stats.xpepelok.me/total&labelColor=64748b)

Менеджер аккаунтов Cristalix. Пока приложение открыто, оно ловит токены аккаунтов, под которыми ты заходишь через лаунчер, и хранит их зашифрованными. Дальше переключаешься между ними в один клик.

## Возможности

- **Запуск в один клик** - запуск аккаунта, закрытие запущенного клиента, вход без аккаунта.
- **Импорт по логину/паролю** - импорт списка аккаунтов, приложение само вводит их в лаунчер и сохраняет токены. Распознаёт неверный пароль, пропускает дубли и уже сохранённые аккаунты, показывает прогресс в фоне, импорт можно отменить в любой момент.
- **Обновление токенов** - одной кнопкой перелогинивает аккаунты, токен которых истекает в ближайшую неделю, и сохраняет свежий.
- **Группы** - объединяй аккаунты в группы и запускай всю группу поочерёдно.
- **Профили настроек** (`binds`, `options`, `optionsof`, `voicechat`) - сохраняются, редактируются и применяются к аккаунту.
- **Инфо об игроке** - головы, 3D-скин, группа, подписка, онлайн-статус, метки и закрепление избранных.
- **Счётчик онлайна** - статистика игроков, которые на данный момент используют менеджер аккаунтов.

## Хранилище

Данные лежат в `%LOCALAPPDATA%\AccountChanger` (Windows) или `~/.local/share/AccountChanger` (Linux, учитывается `XDG_DATA_HOME`):

- `vault.dat` - токены, зашифрованы
- `key` - ключ шифрования, только на Linux (права `0600`)
- `profiles` - профили настроек
- `config.json` - настройки приложения
- `session.json` - сохранённые сессии запуска аккаунтов
- `logs.json` - сохранённые данные о логах

## Безопасность

- **Windows** - токены шифруются через **DPAPI** и привязаны к твоей учётной записи Windows на конкретном ПК.
- **Linux** - DPAPI нет, поэтому токены шифруются **AES-256-GCM**. Ключ генерируется при первом запуске и лежит в `key` с правами `0600`: прочитать его может только твой пользователь. Гарантия та же, что у DPAPI - защита от других пользователей системы, а не от того, кто уже получил доступ к твоей учётке.

## Поддержка платформ

Windows поддерживает всё. На Linux отличается следующее:

| Возможность | Windows | Linux |
| --- | --- | --- |
| Ловля токенов, запуск, группы, профили, инфо об игроке | ✅ | ✅ |
| Лаунчер | обычный `.exe`, JAR, новый `.exe`, свой | JAR, свой |
| Импорт по логину/паролю, обновление токенов | ✅ | ❌ |
| Автозапуск клиента (клик «ИГРАТЬ») | ✅ | ❌ |
| Закрытие окна | сворачивает в трей | сворачивает в панель задач |

Импорт по логину/паролю, обновление токенов и автозапуск клиента работают через Windows UI Automation - приложение находит поля лаунчера, вводит текст и нажимает «ИГРАТЬ». На Linux эквивалента нет: AT-SPI не видит нарисованный лаунчером интерфейс, а эмуляция ввода требует XTEST или привилегированного демона `uinput`. Поэтому на Linux эти кнопки скрыты - заходи в аккаунт через лаунчер вручную, токен подхватится автоматически. Вместо автозапуска клиента используй настройку аккаунта «Автовход».

Трея на Linux нет: в GNOME иконки в трее не показываются без расширения, так что кнопка закрытия сворачивает окно, а приложение продолжает ловить токены в фоне.

## Установка на Linux

Нужна Java (для JAR-лаунчера) и, для нативного окна, GTK3 с WebKit2GTK:

```
# Debian / Ubuntu
sudo apt install default-jre libgtk-3-0 libwebkit2gtk-4.1-0
# Fedora
sudo dnf install java-latest-openjdk gtk3 webkit2gtk4.1
# Arch
sudo pacman -S jre-openjdk gtk3 webkit2gtk-4.1
```

Если WebKit2GTK нет, приложение само откроет интерфейс в окне браузера (Chromium/Chrome/Brave в режиме `--app`).

## Приватность

Приложение отправляет **анонимные** метрики - исключительно чтобы считать онлайн и число пользователей. Отключается в настройках («Отправка метрик»); при выключении актуальный онлайн в приложении не отображается.

## Использование

Запусти `AccountChanger.exe` (или `./AccountChanger` на Linux) - откроется окно приложения. Закрытие сворачивает приложение (в трей на Windows, в панель задач на Linux), оно продолжает ловить токены в фоне. Полностью закрыть - кнопкой «Закрыть», на Windows ещё и через меню иконки в трее.

## Сборка

Нужен Go 1.25+.

### Windows

```
go build -trimpath -ldflags "-s -w -H=windowsgui" -o AccountChanger.exe .
```

### Linux

Нативное окно линкуется с GTK/WebKit через cgo, поэтому собирать нужно **на Linux** - кросс-компиляция с Windows не сработает. Нужны `-dev` пакеты:

```
# Debian / Ubuntu
sudo apt install build-essential pkg-config libgtk-3-dev libwebkit2gtk-4.1-dev
# Fedora
sudo dnf install gcc pkgconf-pkg-config gtk3-devel webkit2gtk4.1-devel
# Arch
sudo pacman -S base-devel pkgconf gtk3 webkit2gtk-4.1
```

```
./build.sh             # нативное окно, webkit2gtk-4.1
./build.sh webkit40    # старые дистрибутивы, где только webkit2gtk-4.0
./build.sh nogui       # без cgo, интерфейс откроется в окне браузера
```

Вариант `nogui` кросс-компилируется откуда угодно и ни от чего не зависит:

```
GOOS=linux GOARCH=amd64 CGO_ENABLED=0 go build -trimpath -ldflags "-s -w" -o AccountChanger .
```

Собрать все варианты с Windows можно через Docker (cgo линкуется внутри контейнера):

```
docker run --rm -v "%CD%:/src" -w /src golang:1.25-bookworm bash -c "\
  apt-get update -qq && apt-get install -y -qq pkg-config libgtk-3-dev libwebkit2gtk-4.1-dev && ./build.sh"
```