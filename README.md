<div align="center">

<img src="assets/icon.png" width="128" alt="AccountChanger" />

# AccountChanger

Менеджер аккаунтов для Cristalix. Сам ловит токены и переключает аккаунты в один клик, без паролей.

[![Скачиваний](https://img.shields.io/github/downloads/xpepelok/cristalix-account-changer/total?color=1fa344&label=%D1%81%D0%BA%D0%B0%D1%87%D0%B0%D0%BD%D0%BE&labelColor=64748b&logo=github&logoColor=white)](https://github.com/xpepelok/cristalix-account-changer/releases)
[![Версия](https://img.shields.io/github/v/release/xpepelok/cristalix-account-changer?color=3b82e8&label=%D0%B2%D0%B5%D1%80%D1%81%D0%B8%D1%8F&labelColor=64748b&logo=github&logoColor=white)](https://github.com/xpepelok/cristalix-account-changer/releases/latest)
[![ci](https://github.com/xpepelok/cristalix-account-changer/actions/workflows/ci.yml/badge.svg)](https://github.com/xpepelok/cristalix-account-changer/actions/workflows/ci.yml)
![Платформа](https://img.shields.io/badge/%D0%BF%D0%BB%D0%B0%D1%82%D1%84%D0%BE%D1%80%D0%BC%D0%B0-Windows%20%7C%20Linux-3b82e8?labelColor=64748b&logo=linux&logoColor=white)
![Сейчас онлайн](https://img.shields.io/badge/dynamic/json?url=https://stats.xpepelok.me/stats&query=%24.online&label=%D1%81%D0%B5%D0%B9%D1%87%D0%B0%D1%81%20%D0%BE%D0%BD%D0%BB%D0%B0%D0%B9%D0%BD&color=3b82e8&labelColor=64748b)

<img src="docs/shot-home.png" width="900" alt="Главный экран AccountChanger" />

</div>

## Зачем это

Каждый раз выходить из аккаунта в лаунчере и вводить пароль от другого долго и неудобно, особенно
когда аккаунтов десяток. AccountChanger висит в трее и запоминает токен каждого аккаунта, под которым
ты зашёл через лаунчер. Потом любой из них запускается одной кнопкой: приложение подставит токен
и откроет игру.

## Установка

Скачай `AccountChanger.exe` со страницы [Releases](https://github.com/xpepelok/cristalix-account-changer/releases/latest)
и запусти. Установка не нужна, это один файл. Новые версии приложение скачивает и ставит само.

На Linux файл называется `AccountChanger-linux-x86_64`. Сначала разреши ему запуск, для лаунчера
ещё понадобится Java:

```bash
chmod +x AccountChanger-linux-x86_64
./AccountChanger-linux-x86_64
```

## Что умеет

- **Запуск в один клик.** Запустить аккаунт, закрыть его клиент, зайти без аккаунта.
- **Импорт по логину и паролю.** Даёшь список аккаунтов, приложение само вводит их в лаунчер и
  сохраняет токены. Замечает неверный пароль, пропускает дубли, показывает прогресс и в любой момент
  отменяется.
- **Обновление токенов.** Одной кнопкой перелогинивает аккаунты, у которых токен скоро истекает.
- **Группы.** Собираешь аккаунты в группы и запускаешь всю группу разом.
- **Профили настроек.** Сохраняешь настройки игры (управление, графика, голосовой чат) и применяешь
  к аккаунту. Есть готовый профиль «Минимальные настройки» для запуска пачкой или на слабом железе.
- **Карточка игрока.** 3D-скин и плащ, группа, подписка, онлайн-статус, метки и избранное.
- **Свой фон.** Любая картинка под интерфейсом, с затемнением и размытием.

<div align="center">
<img src="docs/shot-account.png" width="820" alt="Карточка аккаунта с 3D-скином" />
</div>

## Как пользоваться

1. Запусти AccountChanger и оставь его открытым.
2. Зайди в нужные аккаунты через лаунчер Cristalix как обычно. Каждый вход появится карточкой.
3. Дальше жми на кнопку запуска у нужной карточки.

Если закрыть окно, приложение свернётся в трей и продолжит ловить токены в фоне. Выйти полностью
можно кнопкой «Закрыть» или через правый клик по иконке в трее.

<div align="center">
<img src="docs/shot-settings.png" width="820" alt="Настройки" />
</div>

## Как это работает

```mermaid
%%{init: {'theme':'base','themeVariables':{
  'fontFamily':'Segoe UI, Helvetica, sans-serif',
  'primaryColor':'#23262e','primaryTextColor':'#e4e8ef','primaryBorderColor':'#3a3e48',
  'lineColor':'#3b82e8','edgeLabelBackground':'#1b1e24',
  'clusterBkg':'#191c22','clusterBorder':'#2f333c'
}}}%%
flowchart LR
    subgraph pc [" ваш компьютер "]
        direction LR
        launcher["Лаунчер Cristalix"]
        app("<b>AccountChanger</b>")
        vault[("Зашифрованное<br/>хранилище")]
        game["Игра"]
    end

    launcher ==>|"вход в аккаунт"| app
    app ==>|"токен"| vault
    vault ==>|"запуск в один клик"| app
    app ==>|"подставляет токен"| game

    classDef hl fill:#16233a,stroke:#3b82e8,stroke-width:3px,color:#8fb8f5;
    class app hl;
```

Приложение следит за конфигом лаунчера и забирает оттуда свежий токен после каждого входа. При запуске
аккаунта оно готовит для него отдельный экземпляр лаунчера с этим токеном и открывает игру, поэтому
пароль вводить не нужно, а несколько аккаунтов могут играть одновременно.

## Windows и Linux

Windows умеет всё, на Linux почти всё:

| Возможность | Windows | Linux |
| --- | :---: | :---: |
| Ловля токенов, запуск, группы, профили, карточка игрока | да | да |
| Импорт по логину и паролю | да | нет |
| Обновление токенов | да | нет |
| Автоклик «ИГРАТЬ» | да | нет |

На Linux недостающее делается руками: заходи в аккаунт через лаунчер (токен подхватится сам), а вместо
автоклика включи «Автовход» в настройках аккаунта.

## Безопасность и приватность

Токены хранятся в зашифрованном виде и привязаны к твоему компьютеру: на Windows через DPAPI, на Linux
ключом, который доступен только твоему пользователю. Наружу уходит только анонимная статистика онлайна,
её можно выключить в настройках.

## Сборка из исходников

Нужен [Rust](https://rustup.rs) 1.88 или новее:

```bash
cargo build --release -p accountchanger
cargo test --workspace
```

Готовый файл появится в `target/release/`. На Linux для сборки понадобятся `pkg-config`, `cmake` и
dev-пакеты `libxkbcommon-x11`, `libwayland`, `libvulkan`, `libx11-xcb`, `libxcb`, `libfontconfig`,
`libfreetype`, `libasound2`.

## Лицензия

[MIT](LICENSE)
