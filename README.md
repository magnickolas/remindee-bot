# remindee-bot

Telegram bot for managing reminders.

## Features
- You can set reminders with/without some description on today or other date:
    - `17:30 go to meeting` (today at 5:30 PM)
    - `22.02.2022 02:20:22 palindrome`
    - `6:00` (today at 6:00 AM)
- Periodic reminders can be set with [crontab-like syntax][cron]:
    - `30 7 * * 1-5 go to school` (at 7:30 from Monday to Friday)
    - `45 10-19 * * 1-6 break for 15 minutes` (at 10:45, 11:45, ..., 19:45 from Monday to Saturday)
- Supported commands:
    | Command   | Action                  |
    | --------- | ----------------------- |
    | /commands | List supported commands |
    | /list     | List the set reminders  |
    | /del      | Delete reminders        |
    | /edit     | Change reminders        |
    | /tz       | Select timezone         |
    | /mytz     | Show current timezone   |

## How to use

- As a prerequisite, install [Rust] and SQLite development package.
- Install
    ```console
    cargo install --path .
    ```
- Start the bot
    ```console
    BOT_TOKEN=<your bot token> remindee-bot
    ```
- Send `/start`

[rust]: https://doc.rust-lang.org/cargo/getting-started/installation.html
[cron]: https://en.wikipedia.org/wiki/Cron#CRON_expression
