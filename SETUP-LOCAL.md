# Running betterSkillys on this PC

Runs alongside Doomed Realms (which uses ports 8888, 2050 and 843).

| Part | Port | Started by |
|---|---|---|
| Redis (database, `source/Redis/redis.local.conf`, data in `source/Redis/data/`) | 6379, this PC only | `Run Server.bat` |
| Account server (`source/App`) | 8080 | `Run Server.bat` |
| World proxy (`tools/world_proxy.py`) | 2051 | `Run Server.bat` |
| World server (`source/WorldServer`, settings in `wServer.json`), run from `build/live/a` or `b` | 12051 / 22051, this PC only | `Run Server.bat`, `tools/rolling_restart.py` |

## Play

1. `Run Server.bat`. It rebuilds the servers (needs the .NET 8 SDK) unless one is already running, then opens Redis, Account Server, World Proxy and World Server windows. Run again while they're up, it updates the world server without kicking anyone (see below); the account server keeps running its old build. Close those windows to stop the servers (or `python tools\rolling_restart.py stop` for the proxy and world servers).
2. `Play Client.bat`. This runs the client with the AIR runtime from `tools/airsdk`.

## Client

The client is an Adobe AIR app built from `client/src` with the AIR SDK 32 compiler. Unzip `AIRSDK_Compiler.zip` from airdownload.adobe.com into `tools/airsdk/`.

```
python tools\build_client.py                            # client\bin\client.swf for this PC
python tools\build_client.py 188.141.12.3 --package     # dist\betterSkillys-Client.zip for other players
```

For testing changes without touching the live servers or your own client, `tools/testclient/` has isolated test servers (ports 6380, 8081, 2052) and a separate test client; see `client/AGENTS.md`.

The account server address is compiled into `LocalhostSetup.as`, so the script copies the sources to `build/src` and sets it there. The package is a self-contained Windows app (AIR captive runtime) signed with a self-signed certificate in `build/`.

The Rust rewrite of the client (`client-rs/`, see its AGENTS.md) does everything the AIR client does and also runs in a browser. Its package is built the same way:

```
python tools\build_client_rs.py --account http://188.141.12.3:8080 --package   # dist\betterSkillys-rs-windows.zip
python tools\build_client_rs.py --account http://188.141.12.3:8080 --publish   # dist\client: players' clients update to it
```

Players only need the zip once. Its client keeps itself up to date from `dist\client`, which the account server serves at `/client/`: it looks every five minutes, downloads what changed in the background, and switches to the new version at the player's next portal (or straight away on the character list). `Run Server.bat` publishes on every start; to publish without restarting anything, run the `--publish` line above.

- **Versions:** the version is in `client-rs/Cargo.toml` (`[workspace.package]`). Every change goes under `## Unreleased` at the top of `client-rs/CHANGELOG.md` as it lands, in plain words for players. For each release, raise the version and rename that section to it: publishing refuses a version with no section or with an `Unreleased` one left, and the game's Changelog window shows the file.
- **Signing:** releases are signed with `build/client-update.key`, made on the first publish. Keep it (and back it up): clients only take releases signed with the key they were built with, so a new key means sending everyone a new zip.
- **Rolling back:** `cargo run -p bsk-publish -- rollback --to dist/client --version <x.y.z>` (in `client-rs`) makes an earlier published version the latest again; clients switch to it like any update.

## Updating the world server without kicking anyone

The world proxy owns port 2051 and forwards each player to one of two world servers on this PC,
slot a (port 12051) or slot b (22051), each run from its own build in `build/live/<slot>`.
To put a new world server build live, run `Run Server.bat` again: with the servers running it builds the
current source and switches to it (`rolling_restart.py update`), publishing the client first. By hand:

```
python tools\rolling_restart.py update          # start them if they aren't running, else deploy (what Run Server.bat runs)
python tools\rolling_restart.py deploy          # build source/WorldServer into the free slot and switch to it
python tools\rolling_restart.py status          # which slot is active, and how many players the old one still has
```

- **What players see:** nothing at first. The new server takes the Nexus and every new login at
  once; players in the old server's Nexus move there straight away (a short loading screen).
  Everyone else keeps their world, dungeon runs included, gets a chat message that an update is
  waiting, and switches at their next trip to the Nexus or a realm. After 60 minutes
  (`--minutes`) whoever is left is moved to the new Nexus, with warnings 5 and 1 minutes before.
  The old server then stops by itself.
- **The 2-hourly restart** (`restartTime` in `wServer.json`) works the same way: the server asks
  the proxy, which runs `rolling_restart.py deploy --reuse` (the same build, copied to the other
  slot). Nobody is kicked any more.
- **A second deploy while the old server is still handing over** waits for it to empty;
  `deploy --now` moves its players to the new Nexus first.
- **Only the world server is deployed this way.** The account server can be restarted without
  kicking anyone in game. A message change still needs the client and server shipped together.
- **Logs:** `build/live/<slot>/logs/wServer/`, and the proxy's in `build/live/proxy.log`.
- **Switching over the first time:** the world server started by the old `Run Server.bat` holds
  port 2051 itself. Stop it (this kicks everyone, so ask first), then run `Run Server.bat` again
  (or `python tools\rolling_restart.py up`). Restart the account server too, so it lists the two
  world servers of a hand-over as one.
- **Try it on a test stack:** `python tools\testclient\iso_stack.py --slot N start --rolling`,
  then `python tools\rolling_restart.py --iso N deploy`.

## Letting other people play

- **World server address:** `source/WorldServer/wServer.json` sets `serverInfo.address` to the public IP (188.141.12.3). The account server sends 127.0.0.1 instead to clients on this PC.
- **Firewall:** right-click `Allow Remote Players.bat` and choose **Run as administrator**, once. It opens TCP 8080 and 2051.
- **Router:** forward TCP 8080 and 2051 to this PC.
- **If the public IP changes:** update `wServer.json` and rebuild the package.
- **Play on this PC with `Play Client.bat`**, not the friends' package. The package connects to the public IP, so all of your game traffic goes out through the router and back in (NAT loopback), which loads the router and makes the game lag for players connecting from outside. The account server now sends this PC's LAN address to clients that arrive that way, but the local client avoids the router altogether.

## Admin

Rank is the `rank` field of the account in Redis:

| Rank | Role |
|---|---|
| 0 | Regular |
| 1 | Donator |
| 2 | Supporter |
| 3 | Sponsor |
| 4 | Sandbox |
| 5 | Moderator |
| 6 | Admin |

```
source\Redis\redis-cli.exe hget names MEAPY      # account id for a name (upper case)
source\Redis\redis-cli.exe hset account.2 rank 6
```

The new rank applies from the player's next login. The commands are in `source/WorldServer/core/commands/`.

Full admin powers come from a separate `admin` flag. It passes every command check, and lets the player fire items from any inventory slot (for the admin class weapons) and visit other players' vaults. It is stored as a single byte:

```
echo HSET account.2 admin "\x01" | source\Redis\redis-cli.exe
```

## Code history

The `local` branch is HaloCelsius/betterSkillys, plus this setup, merged with [Aurusenth/betterSkillys](https://github.com/Aurusenth/betterSkillys) (remote `aurusenth`). Aurusenth adds item tiers and glow, an admin class, the Admin Arena and Shaitan dungeons, Battle for the Nexus, a new nexus and vault, and more.

To pick up later changes:

```
git fetch aurusenth
git merge aurusenth/main
```

In `PlayerShootHandler.cs`, the merge keeps HaloCelsius's rate-of-fire and projectile-count checks for players who aren't admins.

## Changes from upstream

- **Assassin poison** (world server): `PoisonEnemy` in `Player.UseItem.cs` ignores defense (it took defense off the total), spreads `TotalDamage` over `DurationMS` from the elapsed tick time with one hit a second (it hit every 1.2 s for a per-second amount, so it ran 20% long), and stops when the enemy dies or leaves the world (a dead enemy's timer used to fire every tick forever). `Enemy.Damage` kills at 0 health: it caps damage at the enemy's health, so the old `Health < 0` check let poison and other ability damage leave monsters alive on 0.
- **Nexus vault chests keep their items:** the shared Vault Chests the nexus map places (four of them) were plain containers, emptied by every world server restart. `NexusWorld.LinkVaultChests` links each to a Redis record (`DbNexusChest`) when the nexus starts: the hash `nexusChests`, fields `items.<x>_<y>` and `datas.<x>_<y>` for the chest's tile. Every change to a chest is saved at once, as vault chests are. Moving a chest in the map starts it empty (its old items stay under the old tile's fields).
- **Servers screen (Rust client 1.1.3)**: the character list has a Servers button again (the "Server:" pill under the characters opens it too). It lists the world servers from `/char/list` with Normal/Crowded/Full and the player count, as the AIR client's `ServersScreen` does; a click picks the server and returns to the characters, and the choice is kept (`preferredServer`). The AIR client's screen (title screen, Servers) is unchanged. Client only.
- `client/src/WebMain.as`: on a first run the map scale setting (`mscale`) is undefined until the player enters the game. The first window resize divided by it and asked for a 0-pixel-wide Stage3D back buffer (Error #3780); it now defaults to 1.
- `source/App`: `char/list` and `app/serverList` send 127.0.0.1 as the world server address to clients on this PC, and this PC's LAN address to clients on the local network or reaching us through our own public IP.
- **Network performance** (world server):
  - Each world thread queues the messages it sends during one pass of its loop and sends them together at the end (`NetworkSendHandler.BeginBatch`/`FlushBatch`). A tick's messages leave in a few TCP segments per player instead of one segment per message.
  - World threads sleep until a player's message arrives or the next tick is due, instead of polling and keeping a CPU core busy for every world with players in it. `Program.cs` asks Windows for 1 ms timer precision so ticks stay 100 ms apart.
  - Fixed byte counting when a send needs more than two chunks, which could drop data and corrupt the stream. A message that doesn't fit in the send buffer now goes first in the next send, so messages stay in order.
  - Quick Edit is turned off in the World Server console window. Clicking in it no longer pauses the server.
  - Per-hit debug prints are gone from the console.
  - Ticks slower than 200 ms are logged as "Slow tick" in `logs/wServer`.
  - The WorldServer and Shared projects are compiled with optimisation, in the Debug build that `Run Server.bat` uses too.
- `wServer.json`: port 2051 instead of 2050, and the public address.
- **Client rendering**, ported from the Doomed Realms client's optimisations and adapted to the GPU (Stage3D) renderer. In the nexus this took the test client from 110 to 191 fps.
  - **Ground chunks** (`Map`, `Square`): static ground tiles are copied into one 128x128 image per 16x16 tiles, and each visible chunk is one textured quad (one GPU draw call) instead of up to 256 tiles. Animated tiles and blended edges are still drawn on their own, on top; their plain texture also goes into the chunk so coasts have no dark seam. A chunk is rebuilt (as a new image, since the GPU caches textures by image) when a tile in it or next to it changes, at most 2 builds per frame, and freed after 10 s unseen (`TextureFactory.disposeTexture`). Clicks and aiming find the tile by inverting the ground transform, and objects drawn flat on the ground get their tile's corners from it.
  - **Tile culling** (`Map.draw`): of the 15-tile circle around the player, only tiles whose centre is on screen (with 2 tiles of margin, 4 below for tall sprites) are drawn.
  - **Batching** (`Renderer`): every plain quad of a frame goes into one vertex buffer, uploaded once, and each run of quads with the same image (or the same atlas page) is one draw call; GPU state that is already bound isn't set again. Shadows are one draw call. This is the GPU counterpart of Doomed Realms' particle batching.
  - **Texture atlas** (`TextureAtlas`): images up to 128 px that stay in use (sprites, wall faces, tile edges) are packed into up to six 1024x1024 textures, so runs of different sprites share a draw call. Tiles drawn on their own draw all base faces first and then all edge overlays, to keep the same images together.
  - **Fullscreen** fills the whole screen: the client works out how far its view reaches and asks the server for that sight radius (`VIEW_RADIUS` message, 15 to 40 tiles; `Player.SightRadius`). Players with a normal window keep 15 and its cost. At 4K the whole view draws at about 78 fps (with ~340 draw calls instead of ~1060).
  - **Projection cache** (`Camera`, `Face3D`): while the camera only moves (no rotation or zoom), wall and tile faces reuse their projected shape and texture matrix, shifted by the camera translation. Tiles also no longer rebuild their texture matrix every frame when their offset hasn't changed (`SquareFace`).
  - **Projectiles** check the distance to each target before anything else.
  - **Flashing objects:** the renderer's atlas check only peeks at an image's one-frame tint (`GraphicsFillExtra.peekColorTransform`); it used to use it up, so objects stopped flashing.
  - **Errors:** `GameSprite` keeps its frame time up to date even when a frame throws, so the player can't run away at the speed of the time since the error.
  - **No garbage per frame:** the game used to create about 4,000 objects every frame at 4K, now about 40. `TextureRedrawer` finds cached images without building string keys, the boss damage text is only set when it changes (it was set and measured once per object in the world, every frame), and `GraphicsFillExtra` no longer makes a new ColorTransform per lookup. A normal-sized window went from 139 to 201 fps.
  - **Performance Stats** (Options) also shows the map draw time per frame (and how much of it is the GPU), the time in tiles, objects and the HUD, the draw calls by kind, and `frame = game + flash`: the game's own time per frame against the AIR runtime's. The runtime needs about 4 ms per frame at 4K to composite the screen, whatever the game does, so about 240 fps is the ceiling there.
  - Not ported, because betterSkillys already has them or they only mattered in Ruffle/Flash: the frame-rate option and render-quality toggle (both exist), projectile hit lists (exist), the fullscreen zoom (betterSkillys has map scale), and the Ruffle fixes (outline glow, chat text, 32-bit float drift).
- **Build warnings** (all projects build with none):
  - `Enemy.OnDeath` was never raised, so `OnParentDeathTransition` (used by Lost Halls monsters) never fired. It is raised on death now, and the transition keeps its flag per monster instead of on the shared behaviour definition.
  - BouncyCastle 1.8.9 (known vulnerabilities) is replaced by BouncyCastle.Cryptography 2.4.0; login password decryption (`RSA.Decrypt`) was checked against .NET's own RSA.
  - Obsolete crypto calls in `Shared/Utils.cs` use their replacements, an unused `APIItem` class with a placeholder secret in `MarketBuyHandler` is gone, and the rest were unused code, duplicate usings and Json.NET-filled fields.
  - `/sell` had a dead check (`item.Tier == null`, but untiered items have tier -1); it was removed and the behaviour is unchanged: untiered weapons, armour, rings and abilities still can't be sold.
- **Blobombs** (`BehaviorDb.Avatar.cs`): the Avatar of the Forgotten King's "Blobs, attack!" phase only taunted; it now orders the 8 `shtrs blobomb maker` objects around its arena to spawn Blobombs, as the Shatters' first boss does.
- **Buying a character slot** (`App/Controllers/AccountController.cs`): the handler awaited a command queued on a Redis transaction before running the transaction, so it hung forever holding the account lock. Being `async void`, it had already answered with an empty success, so the client showed the slot while the server never added it, and logging in failed with "Account in use" until the 45 s lock expired. It now runs the transaction, releases the lock and answers with the real result (success, insufficient funds or account in use).
- **Moves onto no-walk squares** (`MoveHandler.cs`, `Player.RejectMove`): the client only stops a player entering a no-walk square, so it can end up standing on one (it happened on a Sprite World "Space" square). The server disconnected for "NoClipping"; with that disconnect commented out it still dropped the move without counting its tick, so the next move disconnected with "TickIds don't match". It now keeps the server position, counts the move, and sends the player back with a Goto, at most once a second.
- **Taunts from game objects:** `Taunt` assumed its host was an enemy and threw for game objects (spawners, controllers), so their lines were never said. It now works for any host.
- **Monsters without HP:** objects with no `MaxHitPoints` (anchors, spawners, traps, controllers) started at 0 HP and `Enemy.Tick` killed them on their first tick, before their behaviour ran. 139 behaviours were affected, among them the Tomb's boss anchor (which dropped the exit portal at once), the Pentaract, turrets and most dungeon controllers. Only enemies with HP now die at 0 HP; the others end through their behaviour.
- **Missing projectiles:** 358 `Shoot` behaviours named a projectile index that their monster's XML doesn't define, so `Shoot` logged "Doesnt have projectile" and the attack never fired. Each now uses the projectile it most plausibly meant: the Forgotten King's tentacle spirals 4 → 0 (White Bullet, otherwise only in its death burst), the rage rings of Oryx 2 and Oryx Deux 7/8 → 13/14 (the Quiet and Weak spinners), Cyclops God's blade attack 4 → 10 (Blade), the NM Blue Dragon God 4 → 6 (the Red Spinning Fireball it never used), Stheno's spiral 2 → 1 (the blinding bullet), the Red Guardian Bee 4 → 3 (its Ball; the other guardian bees' 4 is a second Ball), Archdemon Malphas and Malphas Deux in their small phase 1 → 0 (fast small bullet), the Royal Cnidarian's rings 0 → 2 (Bubble Wave), F.E.R.A.L.'s spray 0 → 2 (Psychic Attack), and the Wishing Troll, Epic Larva and small Cnidarians to the only projectile each has. `python tools/check_shoot_projectiles.py` lists any such call; it attributes each `Shoot` to the `.Init` it is in, or to every `.Init` that calls its helper method.
- **Nexus Bazaar** (`Exalt/Nexus New Auru.jm`, built by `tools/build_nexus_bazaar.py`): the big sand island south of the town, with the class memorial, couldn't be reached, because the town's dock ended at Dark Water Flowing. A bridge now continues the dock to the island, a road with lanterns leads to the memorial plaza, and nine market pavilions stand around it, with palm trees and beach umbrellas on the open sand. Only stores 5 and 6 (pet generators, large cloths) had stalls on this map, and the other shop lists were empty. Every bazaar shop sells for fame (players have no way to earn gold); prices match the Doomed Realms `/shop` where both sell an item (`MerchantLists.cs`):
  - Dungeon Keys (Store 11): the regular key of each of the 43 dungeons that has a world, one stall each, 10 to 500 fame. Elder and Doomed Keys (Store 12): 20 keys and incantations. Enchanted Keys (Store 4): 18 keys that also turn into a Mystery ST Crate, at twice the key plus 250. Guild, test and reusable keys aren't sold.
  - Stat Potions (Store 1): the eight stat potions, 150 fame (Life and Mana 300). Consumables (Store 2): health and magic potions, tinctures, effusions and drake eggs. Specials (Store 3): XP Boosters, the Loot Drop Potion, Backpack, Vault Chest and Char Slot Unlockers, mystery dyes. Small cloths, clothing dyes and accessory dyes (Stores 7 to 9) rotate through their stalls as the town's large cloths do.
  - Merchant fixes: a stall whose shop had every item out on other stalls was dropped for good (`NexusWorld.SendOutMerchant`); it now waits and tries again. A shop item missing from the game data is removed at start-up instead of becoming a merchant with an invalid item.
- **Nexus fountains:** the four fountains in the nexus's water basins were the frozen winter kind, which give off nothing. They are now the White Fountain, with its spray, and standing within 3.5 tiles of one gives the Healing condition (`NexusWorld.HealByFountains`, renewed every half second). The map change and the healing take effect when the world server restarts.
- **Client updates** (account server): `/client/` serves the published Rust client (`serverSettings.clientFolder` in `server.json`, else the nearest `dist/client` above the server) for players' clients to update from. The release lists are never cached; the files, named by their content's hash, are cached for good. It's there once the account server restarts with this, and `dist/client` exists when it starts (`Run Server.bat` publishes before starting the servers).
- **Quest damage dealers** (`QUEST_DAMAGE_SUBSCRIBE` 89, `QUEST_DAMAGE` 90): a client that asks gets its quest's top three damage dealers and its own damage, as the server counts them (`DamageCounter`), whenever they change (`Player.SendQuestDamage`, checked twice a second). The Rust client shows them under the boss bar. The AIR client never asks, so nothing changes for it; a server without this ignores the request.
- **Boosters:** the XP Boosters and the Loot Drop Potion mark their effect with `<XpBoost/>` or `<LDBoosted/>` and a `<Timer>`, which nothing read, so using one did nothing. `Player.Activate` now starts the boost from them (`<Timer>-1</Timer>`: until level 20). Using an XP Booster while one runs gives it back instead of eating it, and starting one says so. `/boosts` lists an active XP Booster (it only counted ones with a positive time, so one lasting until level 20 never showed) and the time left. Loot tier boosts (`<LTBoosted/>`, the Loot Tier Potion and half of the Lucky Clover) still aren't implemented, so the Loot Tier Potion isn't sold.
- **Dungeons** (world server):
  - Map files are looked up without regard to case (`XmlData.WorldDataCache`). Cave of a Thousand Treasures and the Avatar, Skull Shrine and Grand Sphinx setpieces didn't load before.
  - At start-up the server logs worlds whose map is missing and portal destinations with no world (`XmlData.ValidateWorlds`).
  - New world entry for Wine Cellar (map and Oryx 2 already existed). The Ivory Wyvern world is now `The Ivory Wyvern`, the name its portal uses.
  - Maps that placed the nonexistent "Portal to Nexus" (Ocean Trench, Oryx's Chamber, guild halls) use "Nexus Portal", which `UsePortalHandler` treats as an exit. A portal without a destination shows an error instead of throwing.
  - Loot entries that named missing items were fixed (display names replaced by ids, such as `PriestST1` for Book of Geb) or removed (items not in these XMLs).
  - **Endgame:** when a realm closes, its players are sent to a new Oryx's Castle (`RealmManager.SendPlayersToCastle`) instead of being teleported inside the realm. In the castle, killing both Stone Guardians opens the wall to Janus the Doorwarden (`md dwGenerator`); Janus (new fight) drops the Oryx's Chamber portal, Oryx 1 the Wine Cellar portal.
  - **The Oryx chain** runs from a realm's close to the end of Doomed Realms' content. Janus drops the open Chamber portal; each Oryx after that drops a locked portal to the next stage, which takes an incantation to open (below), and a Realm Portal: Oryx's Castle (Stone Guardians, then Janus) → Oryx's Chamber (Oryx 1) → Wine Cellar (Oryx 2) → Oryx's Sanctuary (Oryx 3, new) → Hash Cellar (Oryx 4) → Doomed Cellar (Oryx 5). Each Oryx's death is announced server-wide.
    - **Difficulty curve:** `BossStats` (`logic/behaviors/BossStats.cs`) sets each boss's health, defense and shot damage from its behaviour, so the stages sit on one curve without changing the client's XML. Shot damage is the XML damage times a scale (`Enemy.DamageScale`, applied in `Shoot`). Health rises by a share of the base for each player after the first. It also clears the random rare/epic/legendary roll quest monsters get (up to triple health), which would break the curve.

      | Stage | Boss | Health (solo) | +per extra player | Defense | Typical shot |
      |---|---|---|---|---|---|
      | 1 | Stone Guardians (each) | 35,000 | 25% | 30 | 85-110 |
      | 2 | Janus the Doorwarden | 70,000 | 25% | 25 | 120 |
      | 3 | Oryx 1 | 100,000 | 30% | 35 | 90-210 |
      | 4 | Oryx 2 | 130,000 | 30% | 40 | 135-240 |
      | 5 | Oryx 3 | 180,000 | 30% | 45 | 170-245, Exalted 195-350 |
      | 6 | Oryx 4 | 240,000 | 30% | 50 | 140-280, rare 410-500 |
      | 7 | Oryx 5 | 320,000 | 35% | 55 | 270-350 |

      **Loot** rises the same way. `python tools/oryx_loot_report.py` works out what one player can expect per kill from the tables, as `Loot.Handle` rolls them: each entry is rolled at its chance times 1 + the player's share of the damage + the loot boosts, so a solo player rolls at double the listed chance and each of three equal players at about 1.6 times (with the 25% server-wide event). For a player in a group of three:

      | Stage | Stat potions | Life / Mana | Potion of Oryx | Gear (tier) | White bag |
      |---|---|---|---|---|---|
      | 1 Stone Guardians (both) | 1.4 | - | - | 0.5 (T10-12) | 0.5% (Ancient Stone Sword) |
      | 2 Janus | 1.9 | - | - | 0.5 (T10-12) | - |
      | 3 Oryx 1 | 2.2 | 0.1 / - | - | 0.6 (T10-12) | - |
      | 4 Oryx 2 | 2.4 | 1 / 0.5 | - | 0.7 (T11-13) | 0.9% (Seal of Blasphemous Prayer) |
      | 5 Oryx 3 | 2.6 | 1 / 1 | 6% | 0.6 (T12-14) | 1.6% (Oryx's Escutcheon) |
      | 6 Oryx 4 | 2.9 | 1.1 / 1 | 13% | 0.6 (T13-15) | 7% (the Oryx set) |
      | 7 Oryx 5 | 3.8 | 1.2 / 1.2 | 24% | 0.6 (T14-15) | 18% (Oryx set, Doom Poison, Sword of Dark Enchantment, Crown) |

      Gear stays at about one piece every two kills while its tier rises. A Potion of Oryx counts as all eight potions (`/forge` makes one from them), so it stays rare. Each piece of the Oryx set (Ring of Oryx, Oryx Hide Armor, Oryx Hide Robe, Oryx Scale Armor, Oryx Tooth Sword, Doom Dirk, Flying Doom Daggers, Tome of Noble Assault) is 0.6% from Oryx 4 and 1.4% from Oryx 5 before the multiplier. Oryx 4 and 5 also drop the Oryx forge materials. The first tables were far richer: with the weekday boost bug below, Oryx 5 gave each of three players a Potion of Oryx every kill and an Oryx set piece or other special item about half the time.
    - **White bags for the Oryx set:** Doomed Realms numbered its loot bags differently, so the Oryx set dropped in brown and blue bags without an announcement. `tools/port_doomed.py` (`BAG_TYPES`) now gives the set, Doom Poison and Sword of Dark Enchantment the white bag (6) and Potion of Oryx and Potion of Power the special-consumable bag (7). Only those lines of the generated XML were updated: a full regeneration also rewrites unrelated parts (Native White Demon moves from ElderObjects.xml to DoomedRealms.xml, Elder Abyss.jm changes), so the committed generated files have drifted from the generator.
    - **Weekend loot boost:** `Loot.Handle` added `wkndBoost` (+30%) to every roll every day, while `/boosts` and the login message only claim it on weekends. It now applies only when `World.isWeekend`. This lowers drop rates everywhere on weekdays, back to what players are told.
    - **Castle:** the Stone Guardians' ring attacks never fired: `RingAttack` only shoots when its radius is 0, and theirs was 20. They now use `Shoot` rings. (Other monsters' `RingAttack`s with a radius are left as they are.)
    - **Oryx 1** cycles minions, an invulnerable dance with ring elements and a spiral, his artifacts, his gaze and a second dance; his second dance used to loop forever with him invulnerable three quarters of the time. Below 20% he grows and chases.
    - **Oryx 2** gains a call for guards at 55% (henchmen and vintners under silencing and weakening spinners) and a faster second phase.
    - **Oryx's Sanctuary** (`tools/build_oryx_sanctuary.py`, `BehaviorDb.OryxSanctuary.cs`): a new map in the Exalt sanctuary art both clients already have: a vestibule, a gallery of nobles, the Court (three officers and three clergy; the Deacon heals the Cleric and Cardinal) and a gate that opens when the court is dead (`O3 Gate Deleter 1`), then the round Sanctum. Oryx 3 cycles missiles, a charge that leaves his wheels behind, a telegraphed stunning ring ("KNEEL!"; stand more than eight tiles away), bombs that burst into spikes and spinners. At half health his three Protectors must die before he can be hurt again; after that he uses the stronger Exalted attacks, and below 20% he hunts players down. His Patricians and Oryx Portals aren't used: their shots put players in stasis, and a player in stasis still takes full damage here.
    - **Oryx 4** (Hash Cellar) and **Oryx 5** (Doomed Cellar) are hand-written (`BehaviorDb.OryxDoomed.cs`) and replace the generated Doomed Realms versions; that Oryx 4 stayed in its first ring phase for good. Oryx 4 is Oryx 1's moveset made harder, with Knights of Doom, six orbiting White Moons and a telegraphed weakening ring. Oryx 5 is invulnerable until the twenty Trees of Doom around him are cut down, with the castle's Stone Guardians beside him (stronger here); he then cycles fire, a blade charge, star spirals and summons of Knights and Trees of Doom (each living tree heals him), turns faster with Fire Boomerangs below 60% and makes a last stand below 20%.
    - Also given behaviours: Guardian Elements 3 and 4, Outer Guardian Elements, White Moons, Knights and Trees of Doom, the Hash Cellar's Jungle Fires (a mild fireball at anyone standing right beside one; they line the path) and the Doomed Cellar's Oryx Guardian Swords.
    - **Portals:** each Oryx drops the locked portal (10 minutes) two tiles to one side and a Realm Portal (2 minutes instead of the default 30 seconds) two tiles to the other, so neither hides under the loot bags.
    - **Incantations:** the locked portal is the `Locked Wine Cellar Portal` object, named by the server for where it leads ("Locked: Oryx's Sanctuary"; `DropPortalOnDeath` `name`, which the Rust client shows). Entering it says which incantation it needs (`Portal.LockedHint`, `UsePortalHandler`). Using that incantation within 25 tiles of it, in that Oryx's dungeon, turns it into the open portal for 5 minutes and tells the dungeon who opened it. This is the XML `ChangeObject` activate effect (`targetId`, `newId`, `onlyInArea`, `customString`), which was never implemented before (`Player.AEChangeObject`); an incantation used in the wrong place, or with no locked portal near, says why and isn't spent. Refused item uses (that, too little mana, a consumable while trading) re-send the slot (`ForceUpdate`), because the client empties it as it asks, and put a popped health or magic potion back on its stack.

      | Oryx | Locked portal to | Incantation | Where to get it |
      |---|---|---|---|
      | 1 | Wine Cellar | Wine Cellar Incantation | Janus (4%), the Nexus Bazaar (125 fame), and as before the Tomb, Ocean Trench, Snake Pit, Candyland and Ivory Wyvern bosses |
      | 2 | Oryx's Sanctuary | Oryx's Sanctuary Incantation (`Oryx Sanctuary Key`) | Oryx 1 (4%), the Nexus Bazaar (300 fame) |
      | 3 | Hash Cellar | Hash Cellar Incantation | Oryx 2 (4%), the Nexus Bazaar (500 fame) |
      | 4 | Doomed Cellar | Doomed Cellar Incantation | Oryx 3 (4%), the Nexus Bazaar (1000 fame) |

      The Oryx Sanctuary Key (a test item that opened the Sanctuary anywhere) became the Sanctuary's incantation in the prod XML (server and client copies). The Hash and Doomed Cellar Incantations opened their dungeon anywhere, like keys; `tools/port_doomed.py` (`INCANTATIONS`) makes them unlock the locked portal instead (only those items' lines of the generated XML were updated). The Wine Cellar Key no longer opens the Wine Cellar from anywhere: it works like a Wine Cellar Incantation (prod XML, server and client copies), and the Bazaar sells the incantation in its place. The Bazaar's Oryx Chamber Key still opens the Chamber directly.
    - **Quest arrow:** monsters with `<Oryx/>` now always win the quest choice (`Player.FindQuest`): the Doomed Cellar's Skull Shrine, an Encounter, outscored Oryx 5, and Oryx 3's XML level is 1.
    - Oryx 3's Protectors spawn on him and fly out to their orbits; tossed ones could fail to land (a toss needs a free target square) and leave the phase empty.
    - **Tested** on the isolated stack with the Rust client: realm close to the Castle, every stage's fight and phases (pushed through with `/smite`), each death's announcement and portals, and walking through Oryx 2's portal into the Sanctuary. Not yet played through by a group without admin help, so the numbers above are a first pass.
  - **Portal sources:** Ghost Ship drops Davy Jones' Locker, the Killer Bee Nest The Hive, Oryx 2 Lost Halls (10%), Lord of the Lost Lands Ice Tomb (25%), Hermit God Cnidarian Reef (25%), the Statues altar Magic Woods (25%). Rock Dragon drops Lair of Shaitan, because Lair of Draconis has no map or world yet.
  - **New boss fights** for bosses that stood still: Mixcoatl and its Boss Totems (Forbidden Jungle), Belladonna, the Masked Party God (Beachzone, no attacks), the Abyss Idols, and Horrific Creation in Mad Lab, whose room opens when Dr Terrible dies (`Mad Lab Open Wall` removes the Blue Torch Walls). Horrific Creation drops the Horrific Sorcerer set (`SorcererST0`-`3`).
  - **Ancient Ruins** (`BehaviorDb.AncientRuins.cs`): every monster now has a behaviour. Cacti are ringed by invisible Cactus Burst traps, Monoliths summon Terracotta Paladins and Knights, and the Sandstone Titan (`Ruins Boss Golem`) fights in three phases: invulnerable while its three Ancient Spirits live, and it wakes the Sand Bombs in its chamber. It drops the golem Trickster set (`TricksterST0`-`3`), Sandstone Seal and the exit portal; the Realm Portal that the map placed beside it was removed. The Grand Sphinx has a 25% chance to drop the Ancient Ruins portal.
  - **Cursed Library** (`BehaviorDb.CursedLibrary.cs`): behaviours for every placed monster; the Eyes of the Edified heal the monsters around them. Avalon the Archivist fights in three phases, wakes the Book Bombs around her desk and tosses candles that spin slowing cyclones. She drops Mark of the Archivist, Scholar's Seal and the exit portal. The Skull Shrine has a 25% chance to drop the Cursed Library portal. Optional event: walking up to one of the four Corruption Triggers summons the Corruption Phantom when none is loose (each trigger fires once); it calls its four wisps at half health and drops Skull of Corrupted Souls.
  - **Fungal Cavern** (`BehaviorDb.FungalCavern.cs`): behaviours for every placed monster; the Mushroom Alchemists heal nearby mushrooms and the loose Red Worm segments act as turrets. The map has no Crystal Worm Mother (she belongs to the Crystal Cavern, which has no map here), so the Mushroom Berserker is the final fight: it drops the Mushroom Tribes set (Tome, Sporous Spray Spell, Fungal Breastplate, Cave Dweller Trap) and the exit portal. The Pentaract has a 25% chance to drop the Fungal Cavern portal.
  - **Parasite Chambers** (`BehaviorDb.ParasiteChambers.cs`): behaviours for every placed monster and hazard (Pulsating Obstacles, Pterasite nests); the rooms are walled off by destructible pD Blockades. The colonies and the boss have defense in the thousands, so they apply ArmorBroken to themselves when they're meant to take damage: Swarm Colonies always, the Nightmare Colony (`pD Boss`) in 8-second exposed windows between 12-second shielded ones, and for good below 30% HP. Its support under the wall fires while it fights. The Infested Chest in the treasure room can be broken once its Blood Slurps are dead. The boss drops the parasite set (`pD Tome`, `pD Scepter`, `pD Spell`, `pD Armor`) and the exit portal. The Cube God has a 25% chance to drop the Parasite Chambers portal.
  - **Secluded Thicket** (`BehaviorDb.SecludedThicket.cs`): the map had no spawn point and no boss, so it now has a Spawn region at the south end and Tezcacoatl the Great Basilisk in the lightning arena to the north. The two Adult Basilisks guard the south clearing; once they die, the braziers open the El Dorado temple's brick doorways (paths through the forest also lead around it). Tezcacoatl fights in three phases, summons basilisks, and drops Tezcacoatl's Tail, Mask of Lightning, Lightning in a Bottle, Staff of the Crystal Serpent, Cracked Crystal Skull and the exit portal. Mixcoatl (Forbidden Jungle) now drops an exit portal and has a 50% chance to drop the Secluded Thicket portal.
  - **Idle monsters given behaviours:** Oryx's Castle's Pets of Oryx (chase and bite) and Living Floors (silence and weaken players who stand near), Wine Cellar's Purple Goo puddles, the second Wizard Puppet in the Puppet Master's Theatre, the Shatters' "S" Glassier Archmages (same as the regular ones) and the Deadwater Docks Macaw.
  - **Tomb of the Ancients** (`BehaviorDb.Tomb.cs`): players start beside the dormant Bes, Nut and Geb statues; five sarcophagi around the tomb are guarded by Beam Priests and Priestesses. Each sarcophagus opens once its own guards (within 15 tiles) are dead, and the statues wake once all five are destroyed. Before, a sarcophagus waited for every priest in the map, the statues woke as soon as the sarcophagi opened (without destroying them), the priests had nothing to orbit, the boss anchor died at once (dropping the exit portal at the start), and the artifacts orbited the wrong bosses. Each boss now summons its own three artifacts (Bes the Pyramid, Nut the Sphinx, Geb the Nile ones), which die with it. The boss fights themselves are the existing port.
  - **Ice Cave / Inner Sanctum** (`BehaviorDb.IceCave.cs`): the Inner Sanctum portal never appeared, because the Ice Cave's `ic boss spawner live` is a game object and can't die (its `Suicide` threw and `TransformOnDeath` never ran); it now spawns the portal itself once every Big Yeti and Snow Bat Mama is dead, then removes itself. Big Yetis throw their boulders (they only followed before). Esben checked for his crosses and shielded kings in the same tick he ordered them, before they existed, so he skipped those phases; he now waits 2 s for them.
  - **Exit portals:** these final bosses had no way back except the nexus and now drop a Realm Portal: Thessal (Ocean Trench), Daichi (Mountain Temple), Belladonna, the Masked Party God, Ghost of Skuld (Haunted Cemetery's last stage), the Ice Tomb chest, the Royal Cnidarian, Gulpord (Toxic Sewers), the Head of Shaitan and Oryx 2 (Wine Cellar).
  - **Daichi the Fallen** (Mountain Temple) stayed invincible forever below 40% HP: his "elements" and "increaseSize" states had no way out. He now grows, puts the Powers to sleep and fights again.
  - Ice Tomb's chest spawner runs on the map's Ice Tomb Boss Anchor. The Magic Woods fountain gate waits for the Fountain Spirit (it waited for a nonexistent "Grim Reaper" and opened at once).
- **Admins handle items like everyone else:** admins can trade with players, drop items into public loot bags, and move items into and out of bags they don't own. Before, trades between an admin and a non-admin were refused or cancelled, everything an admin dropped went into a private soulbound bag, and admins couldn't touch other bags. Soulbound items still follow the usual rules.
- **Skin selection showed every skin twice:** the client parses the class list twice at start-up (the regular classes, then the Doomed Realms ones), and the skin parser, hooked to the same signal, added the whole skin list each time. A class's skin list now ignores a skin it already has (`CharacterSkins.addSkin`). Client only.
- **Admin-only classes:** a class with `<AdminOnly/>` in its XML (Vaelith, "For Realm Creators") is only for admin accounts. For everyone else the account server lists it as unavailable, so the class screen hides it, and the world server refuses to create or load a character of that class ("Class locked"). Before this, any player could pick Vaelith, which can hold any item in any slot.
- **Potion storage:** it can be upgraded from its window: each upgrade costs 500 fame and holds 50 more of every potion (account field `potionStorageLevel`, 50 + 50 per level; `UseStorageHandler` action 4). `/account/grabPotions` sends the capacity after the eight counts, which older clients ignore. Its window opens only once at a time (the button or interact key closes it again), its X has a solid click area, and it closes cleanly; before, every press opened another window on top, so the X seemed not to close it. Potions stored above the limit (an account can hold more than it, from before the limit applied) show as a red "+N" beside the full bar instead of being hidden; they can still be used or withdrawn, but no more of that potion can be deposited until it is back under the limit.
- **Tomb quests:** the closed sarcophagi are quests now, so the quest arrow leads to the nearest one; before, it had nothing to point at until a sarcophagus opened.
- **Shadows on the GPU renderer:** every shadow was drawn as a hard-edged black oval with an uneven alpha (0.01 to 0.3 across its corners), ignoring its colour and gradient. They are now drawn like the software renderer draws them: the shadow's colour, half opaque in the middle and fading evenly to nothing at the edge (`Renderer.as`, `Graphic3D.as`). Coloured shadows, such as candle and brazier glows, now show their colour. Client only.
- **Tomb Defender loot:** a misplaced bracket put its whole loot table (Potions of Life, Tomb key, Ring of the Pyramid, Tome of Holy Protection) inside its winter-only drop, so outside winter it dropped nothing.
- **Random dungeon layouts** (`Shared/terrain/DungeonGenerator.cs`): a world with `<RandomLayout>` in `Dungeons.xml` gets a new layout for every instance, built from the rooms of its own map. The map is cut into rooms (wide areas, with their monsters, objects and walls) and corridors (passages up to `passage` tiles wide); the rooms are joined again in a random tree with straight corridors `corridor` tiles wide. The spawn room is the root and each room holding a `<Goal>` object ends a branch, about 60% of the other rooms lie on the way to the goals and the rest become side rooms. Every room is used once, so the monster count stays the same; monsters standing in the old corridors move into a room next to them, while corridor decoration is left out. If no layout fits in 40 tries, or anything fails, the fixed map is used. Background walled in on every side (the hollow middle of a block of wall) is filled with wall, so it doesn't show as a black hole.
  - Enabled for the Tomb of the Ancients (the boss room is the hub and each of the five sarcophagus rooms ends a branch), Pirate Cave, Spider Den, Snake Pit, Forest Maze, Undead Lair, Abyss of Demons, Toxic Sewers, Woodland Labyrinth, Ocean Trench and Ancient Ruins.
  - Left fixed: Mountain Temple (Daichi moves to fixed map positions), Parasite Chambers (its blockades seal the rooms from the corridors), Cave of a Thousand Treasures (the boulder corridor would be lost), and the Mad Lab, Crawling Depths and Davy Jones' Locker (their bosses sit behind doors and walls their scripts open). Open maps (Sprite World, the Hive, the Ice Cave) don't split into rooms, and the multi-stage dungeons (Oryx's Castle, Lost Halls, the Shatters and so on) depend on their layouts.
- **Rolling world server updates:** the world proxy (`tools/world_proxy.py`) owns port 2051 and forwards each connection by the game id in its Hello: world ids from 1000000 belong to world server slot a, from 2000000 to slot b (`serverSettings.worldIdBase`), anything else goes to the active one. `tools/rolling_restart.py deploy` starts a new world server beside the old one, which then hands its players over at their next Nexus or realm trip (`GameServer.DrainTick`; `Client.Reconnect` sends them to the Nexus with no key, so the proxy picks the new server); see "Updating the world server without kicking anyone" above. The proxy passes on each player's address (message 250, `PROXY_INFO`), and a world server behind it listens on 127.0.0.1 only (`serverInfo.listenPort`). A login whose account another world server still holds asks it to sign the account out (Redis channel `Kick`) and waits up to 5 seconds before "Account in Use". The account server lists world servers that share an address and port as one.

## Content ported from Doomed Realms

- **Commands:** `source/WorldServer/core/commands/doomed/DoomedRealmsCommands.cs` has 29 commands:
  - Everyone: `/swho`, `/getquest`, `/rules`, `/stats`, `/nothing`.
  - Supporter: `/d`, `/say`, `/stars`.
  - Admin: `/level`, `/summon`, `/god`, `/ci`, `/fame`, `/gold`, `/hp`, `/mp`, `/att`, `/def`, `/spd`, `/dex`, `/vit`, `/wis`, `/addeff`, `/remeff`, `/osay`, `/kill`, `/rank`, `/ip`.
- **Items and pets:** `python tools\port_doomed.py --dr "..\Doomed Realms\Doomed Realms"` regenerates them:
  - About 390 items and dyes, 122 pets and their projectiles, written to `custom/DoomedRealms.xml` for both the server and the client.
  - Type codes are 0xD000–0xDFFF. `tools/doomed/types.json` keeps them stable, because characters store items by type.
  - Sprites use copies of the Doomed Realms sprite sheets, registered with a `dr` prefix (`client/src/kabam/rotmg/assets/doomed`).
  - Remote textures are saved in `tools/doomed/remote`.
  - Pets whose id is already used in betterSkillys get a " (Doomed)" suffix and keep their display name.
  - Pet behaviour is in `BehaviorDb.DoomedPets.cs`: PetFollow, plus PetHeal and PetStrike (melee and thrown attacks) with the Doomed Realms amounts and cooldowns. Pets that fired projectiles use PetShoot: the same projectiles, counts and spreads, sent as the owner's server shots (`ServerPlayerShoot`), so the owner's client reports the hits and the damage is the owner's. `ServerPlayerShoot` has a trailing projectile-type byte for this (older clients ignore it and draw projectile 0).
  - The port script writes each pet's damage per second on one enemy and its healing onto the generators' `PermaPet` activates (`petDps`, `petHp`/`petHpMs`, `petMp`/`petMpMs`); the Rust client's tooltip shows them.
  - Doomed Realms items are kept out of the random tier drops.
- **Chat:** public chat no longer needs 2 stars (`PlayerTextHandler.cs`).
- **Classes:** 13 Doomed Realms classes, generated by the same tool:
  - Mechanic, Ascendant, Summoner, Druid, Sharklord, Blademaster, King, Palemaster, Father, Adept and Monk.
  - Pirate and Snowman too. Their ids are "Pirate (Doomed)" and "Snowman (Doomed)" because betterSkillys uses those names for monsters, but they show as Pirate and Snowman.
  - Their abilities are in `Player.UseItem.Doomed.cs`: PearlAbility, BuildTower, Summon, Halo, SamuraiAbility, TomeDamage, PartyAOE, MultiDecoy, MonsterToss and MiniPot. The Halo entity is in `Halo.cs`.
  - Turrets, summons and pearl substitutes are betterSkillys allies, with behaviours generated into `BehaviorDb.DoomedAllies.cs`.
  - Slot types 26 and 28 became 126 and 128, because betterSkillys uses them for pet eggs and lutes. The client names the Doomed Realms slot types in `ItemConstants.as`, which is also what lets their items be equipped.
  - Where Doomed Realms gave a class a starting weapon or ability that doesn't exist or doesn't fit its slot, the class starts with the lowest-tier item of that slot.
  - The class screens also read `DoomedRealms.xml`, the class grid adapts to the number of classes, and class names use DisplayId.
- **Dungeons:** 31 Doomed Realms dungeons plus its shop, generated by the same tool:
  - 13 Elder dungeons: Pirate Cave, Draconis, Abyss, Snake Pit, Spider Den, Sprite World, Tomb and Undead Lair, plus Manor, Ocean Trench, Jungle, Beach Zone and Davy Jones' Locker.
  - Doomed Realms made keys, portals and monsters for those last five but never their maps. Every Elder dungeon but Draconis (which has no betterSkillys map) is now built from betterSkillys' own map of the regular dungeon instead of Doomed Realms' (see the Elder bullets below); Elder Draconis keeps Doomed Realms' map.
  - Doomed Cellar, Hash Cellar, Doom Crystal, Forgotten Island, Zombies, Zombies Minigame, Sheep Herding, Forest Sanctuary, Christmas Oryx's Chamber, Party Cellar, Kitchen, Secret, Battle Arena, Free Battle Arena, Underground Mine, The Gauntlet, Turkey Hunting Grounds and Admin Room.
  - `port_doomed_worlds.py` converts the `.wmap` maps to `.jm` in `source/Shared/resources/worlds/Doomed/` and writes `xml/custom/DoomedRealmsWorlds.xml`.
  - Their portals point at those worlds and their keys create the portals (CreatePortal). Monsters' death portals work as in Doomed Realms.
  - Admins can open any dungeon with `/dungeon <name>`; with no name it lists the Doomed Realms ones. Several of them (Battle Arena, Kitchen, Mine, Gauntlet, ...) have no other entrance.
  - Fixed from Doomed Realms: the Elder Beach Key opened the Elder Jungle.
  - **Elder stats:** Doomed Realms balanced its Elder monsters for its own much stronger characters (10 to 700 times the regular monster's health, 10 or more times its damage, defense in the thousands). The port gives every Elder monster twice its regular version's health, its defense + 10 and 1.3 times its projectile damage instead, and twice the experience (`<XpBonus>2</XpBonus>`, which also doubles the per-kill experience cap, so level 20 characters get it too; fame follows experience). An Elder monster is also a quest where its regular version is (`rebalance_elders` in `port_doomed.py`).
  - **Elder dungeons from betterSkillys maps:** `tools/make_elder.py` (run by `port_doomed.py` first) lists the Elder dungeons built from betterSkillys' own map of the regular dungeon (`DUNGEONS`). For each it generates darker "Elder <id>" copies of every ground and wall the regular map uses, with "Lit 1" to "Lit 6" versions of each walkable ground, onto the client's `ElderAssets8x8.png`, an "Elder <id>" copy, with the Elder stats, of every monster the regular map or behaviours use that Doomed Realms gave no Elder version, and an "Elder <id>" copy of every other object the map places that the behaviours define (spawners such as the Snake Grate, so the Elder version spawns Elder monsters). Its outputs are `xml/custom/ElderGrounds.xml` and `ElderObjects.xml` (and client copies); type codes stay stable in `tools/elder/types.json`. `port_doomed_worlds.py` then builds each map from the regular one with everything swapped for its Elder version, braziers and their light, and a random layout where the dungeon lists goals.
  - **Elder Tomb of the Ancients:** rebuilt from betterSkillys' own Tomb: the same rooms, laid out anew for every visit (`<RandomLayout>`), with every monster swapped for its Elder version, the tiles for dark stone and the walls for aged gold Elder versions (`Elder Sandstone Tile` and co., `Elder Tomb Wall`; generated with their sprites by `tools/make_elder.py`, see below). Elder Braziers stand by the rooms' walls, corners first, about 9 tiles apart. The client has no lighting, so their light is baked into the map: the floor a brazier can see within 10 tiles becomes a lighter, warmer "Lit 1" to "Lit 6" version of its ground, brighter closer to it (`LIGHT_REACH` in `port_doomed_worlds.py`). It stops at walls and turns with the camera. The braziers themselves have no glow, which would show behind walls. Its monsters use the regular Tomb's behaviours (`BehaviorDb.Tomb.cs` builds both) with a quarter shorter cooldowns, and the Elder bosses drop the regular loot plus their Doomed Realms Elder drops. The `Elder Tomb Boss Anchor` drops the exit portal once the Elder bosses are dead; the regular Tomb's anchor also drops the Elder Tomb portal half the time. Before, the Elder Tomb's map was a rough path, its bosses had 1,000,000 HP, 5,000 defense and ten times the damage, and the regular anchor it used dropped the exit portal two seconds in. `port_doomed_worlds.py` builds it.
  - **The other Elder dungeons** (Pirate Cave, Spider Den, Snake Pit, Undead Lair, Abyss, Ocean Trench, Beach Zone, Jungle, Manor, Sprite World and Davy Jones' Locker) are built the same way from the regular betterSkillys map: darker tiles, braziers and their light, every monster swapped for its Elder version, and a new layout every visit where the regular dungeon has one. Each dungeon's behaviour file builds both versions (`PirateCaveBehaviours(db, elder)` and so on, with the helpers in `BehaviorDb.Elder.cs`): the Elder version uses the regular behaviours with a quarter shorter cooldowns and spawns Elder minions, and every Elder monster drops its regular loot plus its Doomed Realms Elder drops. The regular boss drops its Elder dungeon's portal half the time. Before, these used Doomed Realms' maps and behaviours, written for its far stronger characters, and many Elder monsters dropped nothing. Walls a behaviour opens by name (Davy's doors) keep their regular look so their scripts still find them.
  - The Undead Lair map's invisible Doomed Realms "Septavius Summoner" dropped a second Realm Portal (an Elder Abyss portal in the Elder Lair) when Septavius died; it now removes itself, since Septavius drops the portals.
- **Minigames:** `source/WorldServer/core/worlds/impl/doomed/DoomedWorlds.cs` runs the worlds that had code of their own; `WorldManager.CreateNewWorld` asks it first.
  - **Battle Arena / Free Battle Arena:** their portals stand in the nexus, under the Guild Hall portal (paid) and the Vault portal (free). Everyone who enters joins the same game until its countdown ends; after that, or once it closes, the portal opens a new one. Each game starts with a 1-minute countdown, then waves of gods and a boss that gets harder every 5 waves. The paid arena gives fame every wave. When everyone has left, the wave reached goes on `/leaderboard`, stored in Redis as the sorted set `doomed.arenaLeaderboard`.
  - **Zombies Minigame:** defend the tower; each zombie that reaches it damages it. When it falls, everyone gets the fame pot.
  - **Sheep Herding:** 2 minutes to herd sheep into the pen for fame; a black sheep empties the pot.
  - **Zombies:** endless waves over the whole map. Doomed Realms aimed most of its zombies outside this map and never spawned Crawlers; here every zombie lands on open ground, Crawlers included, with a Zombie of Draconis every fifth wave (its unused "special zombie").
  - Monsters report to their world through the WorldEvent behaviour. Destructible walls don't count as monsters.
- **Monsters:** `source/WorldServer/logic/doomed/DoomedRuntime.cs` rebuilds the Doomed Realms behaviour engine on betterSkillys:
  - It covers its composites (If, Cooldown, QueuedBehavior, keys, states), attacks, movement, spawning, events (death, hit, chat) and loot.
  - `port_doomed_monsters.py` copies the 163 monster definitions from Doomed Realms into `DoomedMonsters.cs` nearly verbatim, with object types looked up by id.
  - A behaviour written by hand for a monster replaces the generated one: generated monsters register through `InitDoomed`, which steps aside for an `Init` of the same monster (the Elder Tomb's monsters use this).
  - Elder monsters Doomed Realms gave no behaviour use their regular version's behaviour, which spawns the Elder versions of its minions (e.g. Elder Lord Ruthven). Where only betterSkillys has a behaviour for the regular version, the Elder one shares it (`DoomedAliases`, applied in `BehaviorDb.Initialize`).
  - Loot becomes betterSkillys drops with the same chances; soulbound bags need the same damage, as a share of the monster's HP.
  - Objects whose remote sprite no longer exists borrow the sprite of a betterSkillys object with the same name (e.g. Oryx the Mad God 5), or of `SPRITE_DONORS` in `port_doomed.py` (the Elder Beach portal uses the Beachzone portal's).
- **Economy:** `port_doomed_economy.py` generates `core/commands/doomed/DoomedEconomyData.cs` from Doomed Realms' forge recipes, sell prices and shop lists; the commands are in `DoomedEconomyCommands.cs`.
  - `/forge <slot> <slot> ...` (inventory slots 1-8, backpack 9-16), then `/yes` or `/no`. 256 recipes. Wearing a Forge Amulet halves the price and unlocks the 3 advanced recipes, and the amulet breaks when used. `/forgelist` shows what you can make, `/forgelist <item>` searches, `/forgelist all` lists everything.
  - `/sellfame <slot> ...` sells items for fame (Doomed Realms' `/sell`; betterSkillys' `/sell` is its market).
  - `/shop <name>` opens a shop whose merchants sell one list: abilities, weapons, armor, rings, consumables, dyes, keys and pets for fame, and Doomed Realms' donor shops for gold. With no name it lists the shops. Doomed Realms' default shop sold five starter weapons; here each of its store lists is a shop. Its pet generators had no price; they cost 1250 fame like betterSkillys' own.
  - `/arenas` lists running arenas, `/leaderboard` the best arena waves, `/gleaderboard` the guilds with the most fame.
  - `/afk` and `/pause` toggle Paused when no monster is near (not in arenas).
  - Supporter: `/gear` equips a mid tier item in each of your class's slots, about half way up the regular tiers (T7 weapon, T8 armor, T4 ability and ring). Admin: `/set` (the same), `/giveeff <player> <effect>`, `/message <title> <text>`, `/killallx <name>` (kills with experience), `/grave [name] [0-8]`, `/dragons` (Admin Room), `/openarena [free]` (a Battle Arena portal for 30 seconds; betterSkillys' `/arena` is its own arena).
  - Fixed from Doomed Realms: a recipe accepted the same item twice for two different ingredients, and the forge used to take the items even if they changed between `/forge` and `/yes`.
- **Not ported:** Doomed Realms commands betterSkillys already has (`/who`, `/tell`, `/visit`, `/ban`, `/rename`, `/vault`, guild commands, ...), its multiple backpacks (`/b`, `/bp`), and its group/solo joining, which it had switched off.
- Remote sprites that are a strip of seven frames but are only used as an item (the Elder Draconis Key and three others) showed the whole strip; they now show the first frame (`port_doomed.py`, `DoomedAssets.as`). Client only.

### betterSkillys fixes made for the port

- `World.RemoveFromWorld` now removes every Enemy from the enemy list, as `AddToWorld` adds them. Before, monsters without `<Enemy/>` (like the herding sheep) stayed in it after leaving the world.
- `NexusMerchant` returns to any `IMerchantWorld` (the nexus or the Doomed shop) instead of assuming the nexus.
- Earlier dungeon ports put map regions after Store_9 one slot off (Doomed Realms has 9 store regions, not 10); the maps that use them (Elder Undead Lair, Party Cellar, Sheep Herding, both Zombies maps) are regenerated.
