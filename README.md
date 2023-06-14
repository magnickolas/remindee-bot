[![Stand With Ukraine](https://raw.githubusercontent.com/vshymanskyy/StandWithUkraine/main/badges/StandWithUkraine.svg)](https://stand-with-ukraine.pp.ua)

# remindee-bot

<img src="https://raw.githubusercontent.com/magnickolas/remindee-bot/master/extra/logo/remindee.svg" width="150">

Telegram bot for managing reminders.

## Quickstart

1. Install [Rust].
2. Setup your bot with [@botfather](https://t.me/botfather).
3. Install the crate by running the following command in your terminal:

```console
cargo install remindee-bot
```

4. Start the bot:

```console
remindee-bot --token <BOT TOKEN> --database <FILE>
```

Alternatively, you can use environment variables to specify the token and the database location.

```console
export BOT_TOKEN=<BOT TOKEN>
export REMINDEE_DB=<FILE> # optional
remindee-bot
```

5. Send the `/start` command to the bot and follow its instructions 🤖.

## Setting reminders

The formats descriptions with examples are located at [docs/reminders_formats.md](/docs/reminders_formats.md).

[rust]: https://doc.rust-lang.org/cargo/getting-started/installation.html
