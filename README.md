# betterSkillys

A Realm of the Mad God private server (C#, .NET 8) with two clients: the original Adobe AIR client
(ActionScript 3) and a rewrite in Rust that runs on the desktop and in the browser. It is run on
one PC for a small group of players.

This copy started from [runes02/betterSkillys](https://github.com/runes02/betterSkillys) (a better
NR-CORE/Skilly's 7.0, built on TK-Reborn, with the XMLs of RotMG 1.0.4.0). On top of that it has:

- [Aurusenth/betterSkillys](https://github.com/Aurusenth/betterSkillys) merged in: item tiers and
  glow, an admin class, the Admin Arena and Shaitan dungeons, Battle for the Nexus, a new nexus
  and vault, and more.
- The content of Doomed Realms, another private server: its items, pets, classes, dungeons,
  minigames, commands and economy.
- Many dungeons made playable, random dungeon layouts, rebuilt Elder dungeons and a Nexus Bazaar.
- Server networking and AIR client rendering made much faster.
- The Rust client.

[SETUP-LOCAL.md](SETUP-LOCAL.md) lists every change in detail.

## Play

| Script | What it does |
|---|---|
| `Run Server.bat` | Builds and starts Redis, the account server and the world server, each in its own window (close them to stop). It also builds the Rust client's package for players. |
| `Play Client.bat` | Runs the AIR client on this PC. |
| `Allow Remote Players.bat` | Run once as administrator: opens the firewall for players from outside. |

The servers use these ports. Doomed Realms runs on the same PC with 8888, 2050 and 843.

| Part | Port |
|---|---|
| Redis | 6379, this PC only |
| Account server (`source/App`) | 8080 |
| World server (`source/WorldServer`) | 2051 |

To let friends play, forward TCP 8080 and 2051 on the router and give them a package:

```
python tools\build_client_rs.py --account http://188.141.12.3:8080 --package   # dist\betterSkillys-rs-windows.zip
python tools\build_client.py 188.141.12.3 --package                           # dist\betterSkillys-Client.zip (AIR)
```

On this PC, play with the local client rather than a package: a package goes out through the
router and back in, which makes the game lag for everyone.

## The Rust client

`client-rs/` is a complete rewrite of the AIR client. It uses the same art and the same server
protocol, so it works with these servers unchanged. It does everything the AIR client does
([client-rs/PARITY.md](client-rs/PARITY.md) lists each feature) and adds:

- **Any window size:** the HUD lays itself out, with crisp text, a zoomable minimap with
  player menus, and nearby players' health, magic and gear on hover.
- **Browser play:** the web build runs through `bsk-gateway`, which serves the page and bridges
  the browser to the servers.
- **Speed:** over 1,000 frames a second at 2560×1440 (wgpu: DirectX 12, Vulkan, Metal, WebGPU or
  WebGL2).
- **Condition effects as the AIR client shows them:** icons and rising names over players and
  monsters, blurred, dark or grey views while drunk, blind or paused, and so on.
- **Options:**
  - key bindings;
  - world zoom (90% to 130%) and camera modes;
  - frame rate (vsync, unlocked or capped);
  - particles, shadows, health bars, sound and music.

  They are kept across versions in `%APPDATA%\betterSkillys-rs\settings.txt`, and the login in
  `login.txt` beside it.
- **The rest of the game:** the marketplace, trading, guilds, potion storage, NPC windows,
  emotes, the graveyard, and test flags for scripted runs.

```
cd client-rs; cargo run                                  # desktop, for development
python tools\build_client_rs.py                          # build\client-rs\native
python tools\build_client_rs.py --web                    # build\client-rs\web, served by bsk-gateway
```

See [client-rs/AGENTS.md](client-rs/AGENTS.md) for the gateway, testing and layout.

## Repository

| Path | What |
|---|---|
| `source/` | The servers: `App` (account server), `WorldServer`, `Shared` (game data, XMLs, maps in `resources/worlds`), `Redis` |
| `client/` | The AIR client (`src/`, ActionScript 3) |
| `client-rs/` | The Rust client: `app` (the game), `crates/` (renderer, protocol, session, data, world, net, assets), `gateway`, `bot` |
| `tools/` | Build scripts, the Doomed Realms porting tools, the Elder dungeon generator, the Nexus Bazaar builder, and `testclient/` (isolated test servers and a test client) |

The working notes for each part are [AGENTS.md](AGENTS.md), [source/AGENTS.md](source/AGENTS.md),
[client/AGENTS.md](client/AGENTS.md) and [client-rs/AGENTS.md](client-rs/AGENTS.md).

## Prerequisites

- .NET 8 SDK (the servers)
- AIR SDK 32, unzipped into `tools/airsdk/` (the AIR client)
- Rust stable, with the `wasm32-unknown-unknown` target and `wasm-bindgen-cli` for the browser
  build (the Rust client)
- Python 3 (the scripts in `tools/`)
- Redis is included in `source/Redis`.

## What changed here

The highlights; [SETUP-LOCAL.md](SETUP-LOCAL.md) has the rest.

### Gameplay and content

- **Doomed Realms port:**
  - 13 classes and about 390 items, dyes and 122 pets;
  - 31 dungeons and its minigames (Battle Arenas, Zombies, Sheep Herding);
  - 29 commands, the forge, fame selling and the shops;
  - its monster behaviour engine.
- **Elder dungeons:** rebuilt from this server's own maps, with darker tiles, braziers whose light
  is baked into the floor, Elder versions of every monster and a fresh layout every visit.
  Elder monsters are rebalanced to suit these characters.
- **Random dungeon layouts:** the Tomb, Pirate Cave, Spider Den, Snake Pit, Forest Maze, Undead
  Lair, Abyss, Toxic Sewers, Woodland Labyrinth, Ocean Trench and Ancient Ruins rearrange their
  rooms for every visit.
- **Playable dungeons:**
  - Ancient Ruins, Cursed Library (with the Corruption Phantom event), Fungal Cavern, Parasite
    Chambers, Secluded Thicket and the Tomb of the Ancients;
  - the Ice Cave and Inner Sanctum, and the Oryx's Castle endgame chain;
  - new fights for bosses that stood still (Mixcoatl, Belladonna, the Abyss Idols, Horrific
    Creation and others);
  - exit portals for final bosses, and portal sources for dungeons nothing led to.
- **Nexus:**
  - the Nexus Bazaar, a market island selling keys, potions, consumables, dyes and cloths for
    fame;
  - fountains that spray and heal;
  - Battle Arena portals.
- **Fixes:**
  - 358 monster attacks that named missing projectiles;
  - objects without HP dying on their first tick;
  - buying character slots;
  - XP and loot boosters doing nothing;
  - the skin list showing every skin twice;
  - admin-only classes, admins trading and sharing bags;
  - potion storage upgrades.

### Performance

- **World server:**
  - a tick's messages leave together;
  - world threads sleep instead of spinning;
  - a stream corruption bug in large sends is fixed.
- **AIR client:**
  - rendering ported from the Doomed Realms client: ground chunks, tile culling, batching and a
    texture atlas;
  - fullscreen sees the whole screen;
  - almost no garbage per frame.

  In the nexus it went from 110 to 191 fps, and to about 78 fps at 4K with the whole view in
  sight.
- **Rust client:** about 1,100 fps at 2560×1440.

## Content status (from upstream)

- [x] Removed TK-Reborn bloat, renamed its files, split up Player.cs, labelled entities and worlds.
- [x] XMLs up to date with RotMG 1.0.4.0, client/server asset sorting.
- [x] Fixed what broke when the XMLs were imported (Hermit, Treasure Cave, Davy, Shatters, ...).
- [x] Damage counter, parallax title screen, screens that scale with the window, wis-mod in
  tooltips.
- [x] OnPlayerHit / OnPlayerAbility / OnPlayerShoot activate effects.
- [x] Potion Storage, Marketplace, seasonal drop tables, item breakdowns, animated emotes.
- [x] Samurai and Bard.
- [ ] Every equipment item from RotMG 1.0.4.0 working.
- [ ] The AIR client's memory leak.

### Dungeons and bosses

✮ finished but rough; ✧ map and monsters, still being worked on.

- [x] Garnet/Jade Statues, Killer Bee Nest, Mountain Temple, Toxic Sewers, Hive, Magic Woods,
  Ice Tomb, Tomb of the Ancients.
- [x] Ivory Wyvern ✮, Ice Cave / Inner Sanctum ✮.
- [ ] Lair of Draconis: no map or world here yet (the Rock Dragon drops Lair of Shaitan instead;
  Elder Draconis uses Doomed Realms' map).
- [x] Ancient Ruins, Cursed Library, Fungal Cavern, Parasite Chambers, Secluded Thicket: each has
  behaviours for every monster, a boss with loot and an exit portal, and a portal source.
- [x] Abyss Idol and Horrific Creation: both have fights now.
- [x] Battle for the Nexus ✧, Nest ✧, Lost Halls ✧, Cultist Hideout ✧, The Void ✧, Court of
  Oryx ✧, Oryx's Sanctuary ✧.

## Credits

- **Runes:** made betterSkillys.
- **Slendergo:** created TK-Reborn, which betterSkillys is based on, and helped update the XMLs to
  RotMG 1.0.4.0, among other things.
- **Zolmex:** animated emotes, wis-mod, and many GitHub repositories that went into betterSkillys.
- **Aurusenth:** item tiers and glow, the admin class, the Admin Arena and Shaitan dungeons,
  Battle for the Nexus, the new nexus and vault.
- **Doomed Realms:** the items, pets, classes, dungeons, minigames, commands and client rendering
  optimisations ported here.
- **MIKEEEEEEEEEE:** maps for Mountain Temple, Toxic Sewers, The Nest and Hive.
- **Evil:** behaviours for the Garnet/Jade statues, and pointers to other sources of behaviours.
- **MoonMan:** behaviours for Magic Woods and other entities.
- **animus:** the proxy tool used to rip maps from RotMG Exalt.
- **Boftheb:** helped rip the Fungal Cavern, Secluded Thicket, Lost Halls, Cultist Hideout and
  other maps.
- **Gummy:** the Lair of Draconis map.
- **nekoT:** betterAssets in the client.
- **Orb, GlassBBQ:** credited on TK-Reborn.
