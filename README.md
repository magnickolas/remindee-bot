[![Stand With Ukraine](https://raw.githubusercontent.com/vshymanskyy/StandWithUkraine/main/badges/StandWithUkraine.svg)](https://stand-with-ukraine.pp.ua)

# remindee-bot

<img src="https://raw.githubusercontent.com/magnickolas/remindee-bot/master/extra/logo/remindee.svg" width="150">

Telegram bot for managing reminders.

## Installation

0. Setup your bot with [@botfather](https://t.me/botfather).

### Method 1: Rust's package manager

1. Install [Rust].
2. Install the crate and start the bot:

   ```console
   cargo install remindee-bot
   remindee-bot --token <BOT TOKEN> --database <FILE>
   ```

   Instead of flags you can use environment variables to specify the token and the database location:

   ```console
   export BOT_TOKEN=<BOT TOKEN>
   export REMINDEE_DB=<FILE> # default is to store in the user's data directory
   remindee-bot
   ```

### Method 2: release archive

1. Download the archive for your system architecture from [the latest release page.](https://github.com/magnickolas/remindee-bot/releases/latest)
2. Unpack the archive:
   - for Linux, you can run `tar xf remindee-bot-<version>-<architecture>.tar.gz`;
   - for macOS, you can use the default zip extractor or run `unzip remindee-bot-<version>-<architecture>.zip`;
   - for Windows, you can use the default zip extractor.
3. Navigate to the directory and start the bot:

   ```console
   cd remindee-bot-<version>-<architecture>
   ./remindee-bot --token <BOT TOKEN> --database <FILE>
   ```

### Method 3: Docker container

1. Build the image from this repository:

   ```console
   docker build --tag remindee-bot 'https://github.com/magnickolas/remindee-bot.git#master'
   ```

2. Initialize and run a container from the built image:

   ```console
    docker run -d -e BOT_TOKEN=<BOT TOKEN> remindee-bot
   ```

   You can additionally pass a directory where the database file should be stored with an additional parameter `-v <PATH>:/data`. See [Docker's documentation][docker-docs] for more.

### Method 4: from source

1. Install [Rust].
2. Clone the repository with Git:

   ```console
   git clone https://github.com/magnickolas/remindee-bot
   ```

3. Build the crate and start the bot:

   ```console
   cargo install --path remindee-bot
   remindee-bot --token <BOT TOKEN> --database <FILE>
   ```

## Using bot

Send `/start` command to the bot and follow its instructions 🤖.

## Setting reminders

The formats descriptions with examples can be viewed at [readthedocs] or [docs/index.rst](/docs/index.rst).

You may also find it useful to refer to [the pest grammar playground][pest-grammar-playground] to try out some reminders and see how they are parsed (select `reminder` at the bottom of the list of choices next to the second code block and play with it).

[pest-grammar-playground]: https://pest.rs/?g=N4Ig5gTghgtjURALhAdwM4AIC8mD6wmAOiMaQD5lFEAuJmlJ1E9jI1AdvQL5FccB6AZgC0YzABsAljQCm0CVjEi%2BAT1kI8AVw4zM%2BgznyEAeiVX0AfpgAUZkLKu37UJ3ZItS19yHQkA-ACUQSHBmLwcMAD2HDQAFtq6NIb6uASY9tFu9lxeziR0eT5x2SR%2BIKGV4XyosrIA1ol6KUbp9qilDp2ORfb1neWVwWERACZQqk3JLa2mJKOdrr3mAwHDIdUccVFaEFMzaXMgJcsgUZ1anZ6Y3vaD6%2BubMFIcWnL7hod8hpmdUp25G75XysDIkS6nQpAnw9aF3NZDSp8CLoWQAYxiow%2BBkOYJBp1htxIaM651OgKJIAWp3uiMeYyg72isQSOj0uN%2BpzJcJIFOBUMpJx5%2BLpGwifGZ41Un0wAAEjjBSQDOtThUsgfYLBUHmK%2BDQtLJ0FKcXKjvruqtTqrKerrJqETrNrVRhxDcbcPK8R0CSrlT6ab7Tra8VrRY6IvFdkaJqlTXj4hcrpbhdbgcH7drRZsAGYQKTGk2e%2BzZpOnf5WxZODNZlGM3YFj1HdDBnwC4EQ4XXSmpnzplaZnUjPjoHQF2NFsodyl8nw9lxV-uIzZCUTKNfr5R8Pgr5SYGhSGCyTAAByiL2SbJoSjEfDHje%2BBgA5ABGR8AOjfj4ATI%2BgQBBABlABhABJEC8AAERAgBxECABUHzYABmJxHwABnfT9X0Qsg0ICf9gLAvAADkAHliIALQAUQAJVIyCYPg5EJRieIjHlB99BIZ9UIwj9v0fHCSDwioCNA8CyMo2j6Kg2CEI4CJ1AQcdCEA8SGLk4AABZuE2Phtl2GYWnvDhDBIL9eMwx8kME0yGEwdCrNfMSiNkpi7MoNTXMY%2BSImeV53iiPZUQxDhRlmBy%2BM-ABWX9rC88C3Lg%2ByEo0%2BCnheN4jyMlTMH8rK8CCvAQsxTYSrCnLcvywLgvRUrxQ4WoGndYxOLyzEJhw-U3U6jzMGdV1owsPrIwgIacNzfNev0Shm31CApRwkcwt6iIdw3DbNpvfh1pETA4i0eAOBECANHGAAjCQjxHMBlM3XRYnkAA3KAJDwUYpCeqR0CKuIIGPU7sykAAPCKbBIARQXsWQnvkLVAiBDAACpNn3Q93s%2B77fv%2BwGQZSXFwZASGKDxGG4ZIBHrGR1GD1kPA-oB2QgdByrYzaEhGXwqn0BRiIMR0GhRiiVAOHpnGmbxg5jDTbM5GuSh7BeUESAAagp-xEZ5p06nqKV0Axr6fr2BncdBgmIahkgyYgLV7PsGIKc13nh2PaQaDl1mpfSEhABQCZWQBvEmSBUUg1uELaI62rdBHD8Rz2e17VxUB65ZeiRPZlAASYA2vjiA07wDEYFPV1YjVhS%2BDzgui5L2RYmM1q7P0KvXsLqJi5iOuaDwJSxpwlu3przvYjwZl4j8PqB7bjvS%2B7pr6gnmbMCnofZ-eiZF-slf29rkeDL7yfHvz1vV670fMrkTfKG3mez-K0YJ4jWm8AH1ncGztq0bpm-d5ocun-Rj-YeyQvhN2XkfauO9gH0x2AfJeQC17VUNP3CBJ8oFr3vo-W8jJv6oPTpVd%2BOcwHjHeAgru-9sGkLwdPX%2Bsxc7UNPiPXuV9MAkNwanNBt8R5jziCwshI9558IYegs%2BetmL8H4d3ZhXtCBT2kdYXu%2BwIiSNHqxXhDd0hTx4VgawPClGV2EVwueOssAaNkdQwRiMdb6JTgnQeIiR56yMriKeTjrBShsSo-epj8bSynt4oE%2B9PGGN-ufAKhpfGaOoUgnReUL500vJsFRmDInmI4W9FJ1h742LYS-EJ0DtERS0Wo2JuS9GJIalPNOBpCyqUIhJci1E6JpWSvFepLTna2OPm9aRuUqmvRqco6JJS%2BnUOqUeIZ6S8CWNGVM8ZSTqFONqeAuZAyJkGKmQE2Zdi8DzMmTsmJ7E0k7L2RsnZKTGwrJOWs5csdI73W3Hc1hODMBQAql-TA2YgrwCvEnR5ry0RolkMeZI50ohsV7gIHhAhjRvPClKN8PC3y90%2Bd8xkE82F0LATYGwKLrAWxAGEXRaigQEqJc86U1gABkNh0CuxkB7NgxN7IYECDhY03gSBvicDw6EXKnC93WOIj5oDDD7z5cgHl8SJVICcPfIVFcY5J3uSq5O-zdyYrhXuWmrDMZG2vMnXJH1DZBSxYYVxersaM2ZkCXJA8cLzz1gbLGxtxY2qpjrJ10AOBgGQYqr%2BzqjZmoMAG41LqxbWrxtYAN9rFW7VVSq6Ou1nlyFee8nV3rfUGsoXTXM7cG7ZxTesla7wBZSHwSaQtmKGSXzwKec8WdCDVpzfrTN2VYwf2ITgwNQV2XdrzTADWWT6Xu3kLa7tZaJBDtsNTdx3aw1G2CH294A6nbjveAu3tiq2H62PIyOWdlRUGB3XgNty7DR1rPLEcRJ692jogBwLAR79C3v3fIUynKQAABonCvvvRwQISNxF6mfquxt2rDw03RpO8DHyAF03rfXSthA4MgfRm2xtn9n6bpYCNUDEB27TrpW7D20bn6TunbOiDdMcNdXw-m7mKMyPo1o-65%2Bd6D1syIYYANZ68Po0Q3QNjAm30PqfY3Hjz8TYS1BsxhDomP22BID%2BvIAaOPvsA8BxqnqJh4DAx2wgjqYwRCM5MGgUQC2GZ09KEz1nW1vN9QZtqpm9MEZgBKkOVjmq6fM0uxVpn7M%2Boic%2B-qdnT0OaPJ%2BlTXndYb3C0FzTca7kJojkmp5p1-KjDHepsTfylU0owPZKipEQIIwkFEKIC9XlxDOnuCzHBwV5UZGiEoSqpDZj3DV06rzusNcwHSqAQKsDnVkDQJqplTpol2KdDgQK03wsNGiPMIKpAOw4JN6bdc5uWban%2Bg9sSMAqyBGphTsSCtYEoMV0rdGRP-rOzYQrl2StssVfzWIQsRaFVdDKAgbU3uC2FqLaT7qrndPEf9j7YDMNgIh4Dr7kWlPfqcLDz7ndANAnO0V574iMsvCy3sHLpkQsbYgDNoFOEUf8DDsqlL640tJ1YYt5b%2B4Yh5ZXD8lrmAGvHVkMXGg0pUQAEcDSzaPFEDrqAgoP3%2BTBmg0By0vDAP1OIMhDR7rm0LkXc2bCyGBmiCQWh0CfVkC9rL6AltSBWzEaZUuwYAEJCvxWIgATUCEdsYTPLcs6h0cxn5vmerdFpLha0IqNm4t1boPUv0fU9p9tPguOwpjsIW1ACJW13WET-jutCmM9%2B4j97yjWtrBXfEd%2BkALxjxvGQCAAAHEgNCaF%2BpQHqEeLQx5MBIECCAbgQA#editor
[readthedocs]: https://remindee-bot.readthedocs.io/en/latest/
[docker-docs]: https://docs.docker.com/
