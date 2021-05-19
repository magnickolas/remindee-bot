# remindee-bot

Telegram bot for managing reminders.

## Features
- You can set reminders with/without some description on today or another date:
    - `17:30 go to meeting` (today at 5:30 PM)
    - `22.02.2022 02:20:22 palindrome`
    - `6:00` (today at 6:00 AM)
- Some fields (day, month, year) can be omited depending on the current time:
    - `8:00 wake up` (if set at e.g. 10 PM, the bot'll remind at 8 AM tomorrow)
    - `1 0:00 ++month` (the bot'll remind at 12 AM on the first day of the next month) 
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

## Quickstart

1. Install [Rust].
2. Setup your bot with [@botfather](https://t.me/botfather).
3. ```console
   BOT_TOKEN=<your bot token> cargo run --release
   ```

## How to use

- As a prerequisite, install [Rust].
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
