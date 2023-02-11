# remindee-bot

<img src="https://raw.githubusercontent.com/magnickolas/remindee-bot/master/extra/logo/remindee.svg" width="150">

Telegram bot for managing reminders.

## Quickstart

1. Install [Rust].
2. Setup your bot with [@botfather](https://t.me/botfather).
3. Install the crate:

```console
cargo install remindee-bot
```

4. Start the bot:

```console
remindee-bot --token <BOT TOKEN> --database <FILE>

# Alternatively, one can use environment variables:
export BOT_TOKEN=<BOT TOKEN>
export REMINDEE_DB=<FILE> # optional
remindee-bot
```

5. Send `/start` and follow the machine's instructions 🤖.

## Features

- You can set reminders with/without some description on today or another date:
  - `17:30 go to restaurant` => notify today at 5:30 PM
  - `01.01 0:00 Happy New Year` => notify at 1st of January at 12 AM
- Some fields (minutes, day, month, year) can be omitted depending on the current time:
  - `8 wake up` (the bot will remind at nearest 8 AM)
  - `1 0:05 ++month` (the bot will remind at 12 AM on the first day of the next month)
- Periodic reminders can be set with [crontab-like syntax][cron]:
  - `55 10 * * 1-5 go to school` (at 10:30 AM every weekday)
  - `45 10-19 * * 1-6 break for 15 minutes` (at 10:45, 11:45, ..., 19:45 from Monday to Saturday)
- Supported commands:
  | Command | Action |
  | ------- | ----------------------- |
  | /help | List supported commands |
  | /list | List the set reminders |
  | /del | Delete reminders |
  | /edit | Change reminders |
  | /tz | Select timezone |
  | /mytz | Show current timezone |

[rust]: https://doc.rust-lang.org/cargo/getting-started/installation.html
[cron]: https://en.wikipedia.org/wiki/Cron#CRON_expression
