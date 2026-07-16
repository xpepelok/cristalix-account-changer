# AccountChanger

[![Скачиваний](https://img.shields.io/github/downloads/xpepelok/cristalix-account-changer/total?color=1fa344&label=%D1%81%D0%BA%D0%B0%D1%87%D0%B0%D0%BD%D0%BE&labelColor=64748b&logo=github&logoColor=white)](https://github.com/xpepelok/cristalix-account-changer/releases)
[![Версия](https://img.shields.io/github/v/release/xpepelok/cristalix-account-changer?color=3b82e8&label=%D0%B2%D0%B5%D1%80%D1%81%D0%B8%D1%8F&labelColor=64748b&logo=github&logoColor=white)](https://github.com/xpepelok/cristalix-account-changer/releases/latest)
![Платформа](https://img.shields.io/badge/%D0%BF%D0%BB%D0%B0%D1%82%D1%84%D0%BE%D1%80%D0%BC%D0%B0-Windows-3b82e8?labelColor=64748b&logo=windows&logoColor=white)
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

Данные лежат в `%LOCALAPPDATA%\AccountChanger`:

- `vault.dat` - токены, зашифрованы через Windows DPAPI (привязаны к учётке Windows)
- `profiles` - профили настроек
- `config.json` - настройки приложения
- `session.json` - сохранённые сессии запуска аккаунтов
- `logs.json` - сохранённые данные о логах

## Безопасность

Токены шифруются через **Windows DPAPI** и привязаны к твоей учётной записи Windows на конкретном ПК.

## Приватность

Приложение отправляет **анонимные** метрики - исключительно чтобы считать онлайн и число пользователей. Отключается в настройках («Отправка метрик»); при выключении актуальный онлайн в приложении не отображается.

## Использование

Запусти `AccountChanger.exe` - откроется окно приложения. Закрытие сворачивает в трей, приложение продолжает ловить токены в фоне. Полностью закрыть - кнопкой «Закрыть» или через меню иконки в трее.

## Сборка

Нужен Go 1.25+, собирается под Windows:

```
go build -trimpath -ldflags "-s -w -H=windowsgui" -o AccountChanger.exe .
```