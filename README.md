# remindee-bot

Telegram bot for managing reminders.

## Features
- You can set reminders with/without some description on today or other date:
    - `17:30 go to meeting` (today at 5:30 PM)
    - `22.02.2022 02:20:22 palindrome`
    - `6:00` (today at 6:00 AM)
- There is crontab syntax support for periodic notifications:
    - `30 7 * * 1-5 go to school` (at 7:30 from Monday to Friday)
    - `45 10-19 * * 1-6 break for 15 minutes` (at 10:45, 11:45, ..., 19:45 from Monday to Saturday)

## How to use

- As a preprequisite, install [Rust] and SQLite development package.
- Install
    ```console
    cargo install --path .
    ```
- Start the bot
    ```console
    TELOXIDE_TOKEN=<your bot token> remindee-bot
    ```
- Send `/start`

[rust]: https://doc.rust-lang.org/cargo/getting-started/installation.html
