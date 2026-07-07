# Spoticord (personal single-account build)

Spoticord is a Discord music bot that allows you to control your music using the Spotify app.
Spoticord is built on top of [librespot](https://github.com/librespot-org/librespot), to allow full control using the Spotify client, with [serenity](https://github.com/serenity-rs/serenity) and [songbird](https://github.com/serenity-rs/songbird) for Discord communication.
Being built on top of rust, Spoticord is relatively lightweight and can run on low-spec hardware.

> **This is a stripped-down, single-account fork.** The multi-user machinery
> (PostgreSQL database, the `spoticord-link` account-linking frontend, Redis
> stats) has been removed. The bot plays from **one** Spotify account, which it
> signs into with a one-time OAuth login. Everyone in a voice channel shares
> that account.

## How to use

The quickest way to run it is the bundled Docker stack:

```sh
cp .env.example .env      # then fill in DISCORD_TOKEN
docker compose up -d
docker compose logs -f    # on the first run, open the Spotify URL printed here
```

On the **first launch** the bot has no cached credentials, so it prints a Spotify
authorization URL. Open it once in a browser and approve access; the reusable
credentials are then saved to the `spoticord-data` volume (the `/data` directory)
and reused on every later start — no further login needed.

### Environment variables

Only one variable is required:

- `DISCORD_TOKEN`: The Discord bot token used for authenticating with Discord.

Optionally you can configure:

- `DEVICE_NAME`: The Spotify Connect device name the bot advertises. Defaults to `Spoticord`.
- `CACHE_DIR`: Directory where the reusable Spotify credentials (`credentials.json`) are stored. Defaults to `/data`.
- `GUILD_ID`: The ID of the Discord server where this bot will create commands for. This is used during testing to prevent the bot from creating slash commands in other servers, as well as generally being faster than global command propagation. This variable is required when running a debug build, and ignored when running a release build.

#### Providing environment variables

You can provide environment variables in a `.env` file at the root of the working directory of Spoticord.
You can also provide environment variables the normal way, e.g. the command line, using `export` (or `set` for Windows) or using docker.
Environment variables set this way take precedence over those in the `.env` file (if one exists).

# Compiling

For information about how to compile Spoticord from source, check out [COMPILING.md](COMPILING.md).

# Contributing

For information about how to contribute to Spoticord, check out [CONTRIBUTING.md](CONTRIBUTING.md).

# Contact

![Discord Shield](https://discordapp.com/api/guilds/779292533053456404/widget.png?style=shield)

If you have any questions, feel free to join the [Spoticord Discord server](https://discord.gg/wRCyhVqBZ5)!

# License

Spoticord is licensed under the [GNU Affero General Public License v3.0](LICENSE).
