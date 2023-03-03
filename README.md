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

### One-time reminders

The format of reminder consists of three parts: `<date> <time> <description>`:

- Date is in either `day.month.year` or `year/month/day` formats
- Time is in format `hour:minute`

**Examples:**

- You can set reminders on specific date and time:
  - `01.01 0:00 Happy New Year` => notify at 1st of January at 12 AM
- Some fields (day, month, year, minute, second) can be omitted. In this case the notification time will be sanely derived as the nearest time point in the future:
  - `8 wake up` (notify at 8 AM **today** if it's 0:00-7:59 AM, **tomorrow** otherwise)
  - `1 0 ++month` (notify on the first day of the next month at 12 AM)

### Recurring reminders

Recurring reminders is an extended format, thus everything from the previous section applies here. Now there are three parts `<date pattern> <time pattern> <description>`:

- Date pattern is in either `date` or `date_from-date_until/date_divisor` formats (can specify multiple with `,` separator)
- Date divisor is in either `1y2m3d` or `mon-tue,thu,sat-sun`-like formats
- Time pattern is in either `time` or `time_from-time_until/time_divisor` formats (can specify multiple with `,` separator)
- Time divisor is in `1h2m3s`-like format

**Examples:**

- Notify every one and a half hours from 10 AM to 8 PM on weekdays:
  - `-/mon-fri 10-20/1h30m take a break`
  - `On Monday-Friday at 10-20 every 1hour30mins take a break`
- Notify on every Sunday from the 1st of April to the 1st of May at 15:00:
  - `1.04-1.05/sun at 15:30 clean the room`
  - `01.04-01.05 every Sunday at 15:30 clean the room`
- Notify on the 20th day of every month at 10 AM:
  - `20/1m 10 submit meter readings`

### Countdown

This kind of reminders is in `<duration> <description>` format and sets a timer for the specified duration.

- Duration is in `1y2mo3w4d5h6m7s` format

**Examples:**

- Notify after 5 minutes:
  - `5m grab tea`
  - `In 5mins grab tea`
  - `after 5minutes grab tea`

### Cron

_NOTE: Originally cron-like reminders were the only way to create a recurring reminder, but one can still use them with [crontab-like syntax][cron]:_

- `55 10 * * 1-5 go to school` (at 10:30 AM every weekday)
- `45 10-19 * * 1-6 break for 15 minutes` (at 10:45, 11:45, ..., 19:45 from Monday to Saturday)

[rust]: https://doc.rust-lang.org/cargo/getting-started/installation.html
[cron]: https://en.wikipedia.org/wiki/Cron#CRON_expression
