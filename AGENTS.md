# betterSkillys: working notes

A Realm of the Mad God private server (C#, .NET 8) and its Adobe AIR client (ActionScript 3), run
on this PC for a small group of players. Read the guide for the part you're changing:

- [client/AGENTS.md](client/AGENTS.md): the AIR client (rendering, UI, messages, test client).
- [client-rs/AGENTS.md](client-rs/AGENTS.md): the Rust rewrite of the client (desktop and
  browser), in progress.
- [source/AGENTS.md](source/AGENTS.md): the servers (account server, world server, behaviours,
  worlds, Redis data).
- [SETUP-LOCAL.md](SETUP-LOCAL.md): how to run and share the game, and the list of changes made
  here. Add to its "Changes from upstream" or "Content ported from Doomed Realms" section when you
  change behaviour or add content.

## What runs where

| Part | Port | Notes |
|---|---|---|
| Redis | 6379 (this PC only) | `source/Redis`, data in `source/Redis/data/` |
| Account server (`source/App`) | 8080 | |
| World proxy (`tools/world_proxy.py`) | 2051 | forwards to the world servers; see source/AGENTS.md |
| World servers (`source/WorldServer`, from `build/live/a` and `b`) | 12051 / 22051 (this PC only) | public address in `wServer.json`; deploy with `tools/rolling_restart.py` |
| Isolated test servers | 6380 / 8081 / 2052 | `tools/testclient/iso_stack.py`, under `build/iso/` |

Doomed Realms, another server on this PC, uses 8888, 2050 and 843. Don't take those ports.

`Run Server.bat` builds and starts Redis, the account server and the world server (behind the world
proxy), each in its own window; run while they're up, it deploys the world server without kicking
anyone (`tools/rolling_restart.py update`). It skips any of them whose process name (`redis-server.exe`, `App.exe`,
`WorldServer.exe`) is already running, so never run test copies under those names
(`iso_stack.py` doesn't). Players use `Play Client.bat` on this PC, or the packaged client in
`dist/` (`dist/betterSkillys-Client.zip` for friends).

## Ground rules

- **Real people play on the live servers.** Restarting or stopping them disconnects everyone:
  ask first, every time. Test on the isolated stack instead.
- **Never drive the real client window** (title "betterSkillys"); it may be someone's game. The
  test client has its own window ("betterSkillys port test") and its own settings.
- **The client and server ship together** when a message changes (ids, fields). An old client
  must keep working with a new server where possible. Note in the commit which side needs
  redeploying.
- **Secrets** stay in git-ignored `*.local.txt` files (`test-account.local.txt`) and `build/`
  (signing key). `build/`, `dist/`, `client/bin/`, `tools/airsdk/` and `source/Redis/data/` are
  not tracked.
- **Changelog:** every change players can notice (client, servers, content, fixes) gets a line
  in [client-rs/CHANGELOG.md](client-rs/CHANGELOG.md) under `## Unreleased` at the top, in the
  same commit as the change. Write it in plain words for players, not code names; the game's
  Changelog window shows it. Start the section if it isn't there. Publishing a client version
  renames it to that version.
- **Generated files** come from the `tools/port_doomed*.py` scripts (see source/AGENTS.md). Change
  the script and regenerate rather than editing the output by hand.

## Git

- Work on the `local` branch. It is HaloCelsius/betterSkillys merged with Aurusenth/betterSkillys
  (remote `aurusenth`), plus the local setup and the Doomed Realms port.
- Another session may be editing the same tree. Stage and commit explicit paths only (never
  `git add -A` or `git commit -a`), and check `git status` for changes that aren't yours.
- `core.autocrlf` is on: the repository stores LF, while many working files are CRLF. Scripts
  that patch source text should detect and keep each file's line endings.

## Tools

- Python 3 for the scripts in `tools/`.
- The .NET 8 SDK for the servers.
- AIR SDK 32 for the client, unzipped into `tools/airsdk/`.
- Rust (stable, with the `wasm32-unknown-unknown` target) for `client-rs/`.
