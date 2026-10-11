# Running betterSkillys on this PC

Runs alongside Doomed Realms (which uses ports 8888, 2050 and 843).

| Part | Port | Started by |
|---|---|---|
| Redis (database, `source/Redis/redis.local.conf`, data in `source/Redis/data/`) | 6379, this PC only | `Run Server.bat` |
| Account server (`source/App`) | 8080 | `Run Server.bat` |
| World server (`source/WorldServer`, settings in `wServer.json`) | 2051 | `Run Server.bat` |

## Play

1. `Run Server.bat`. It rebuilds the servers (needs the .NET 8 SDK) unless one is already running, then opens Redis, Account Server and World Server windows. Close those windows to stop the servers.
2. `Play Client.bat`. This runs the client with the AIR runtime from `tools/airsdk`.

## Client

The client is an Adobe AIR app built from `client/src` with the AIR SDK 32 compiler. Unzip `AIRSDK_Compiler.zip` from airdownload.adobe.com into `tools/airsdk/`.

```
python tools\build_client.py                            # client\bin\client.swf for this PC
python tools\build_client.py 188.141.12.3 --package     # dist\betterSkillys-Client.zip for other players
```

For testing changes without touching the live servers or your own client, `tools/testclient/` has isolated test servers (ports 6380, 8081, 2052) and a separate test client; see `client/AGENTS.md`.

The account server address is compiled into `LocalhostSetup.as`, so the script copies the sources to `build/src` and sets it there. The package is a self-contained Windows app (AIR captive runtime) signed with a self-signed certificate in `build/`.

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
  - Pet behaviour is in `BehaviorDb.DoomedPets.cs`: PetFollow, plus PetHeal and PetStrike with the Doomed Realms amounts and cooldowns. Pets that fired projectiles hit the nearest enemy directly for that projectile's damage.
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
  - Doomed Realms made keys, portals and monsters for those last five but never their maps. Its other Elder maps are the regular map with each object swapped for its "Elder" version, so these five are built the same way from the regular Manor, Ocean Trench, Jungle, Beach Zone and Davy's Locker maps.
  - Doomed Cellar, Hash Cellar, Doom Crystal, Forgotten Island, Zombies, Zombies Minigame, Sheep Herding, Forest Sanctuary, Christmas Oryx's Chamber, Party Cellar, Kitchen, Secret, Battle Arena, Free Battle Arena, Underground Mine, The Gauntlet, Turkey Hunting Grounds and Admin Room.
  - `port_doomed_worlds.py` converts the `.wmap` maps to `.jm` in `source/Shared/resources/worlds/Doomed/` and writes `xml/custom/DoomedRealmsWorlds.xml`.
  - Their portals point at those worlds and their keys create the portals (CreatePortal). Monsters' death portals work as in Doomed Realms.
  - Admins can open any dungeon with `/dungeon <name>`; with no name it lists the Doomed Realms ones. Several of them (Battle Arena, Kitchen, Mine, Gauntlet, ...) have no other entrance.
  - Fixed from Doomed Realms: the Elder Beach Key opened the Elder Jungle.
- **Minigames:** `source/WorldServer/core/worlds/impl/doomed/DoomedWorlds.cs` runs the worlds that had code of their own; `WorldManager.CreateNewWorld` asks it first.
  - **Battle Arena / Free Battle Arena:** their portals stand in the nexus, under the Guild Hall portal (paid) and the Vault portal (free). Everyone who enters joins the same game until its countdown ends; after that, or once it closes, the portal opens a new one. Each game starts with a 1-minute countdown, then waves of gods and a boss that gets harder every 5 waves. The paid arena gives fame every wave. When everyone has left, the wave reached goes on `/leaderboard`, stored in Redis as the sorted set `doomed.arenaLeaderboard`.
  - **Zombies Minigame:** defend the tower; each zombie that reaches it damages it. When it falls, everyone gets the fame pot.
  - **Sheep Herding:** 2 minutes to herd sheep into the pen for fame; a black sheep empties the pot.
  - **Zombies:** endless waves over the whole map. Doomed Realms aimed most of its zombies outside this map and never spawned Crawlers; here every zombie lands on open ground, Crawlers included, with a Zombie of Draconis every fifth wave (its unused "special zombie").
  - Monsters report to their world through the WorldEvent behaviour. Destructible walls don't count as monsters.
- **Monsters:** `source/WorldServer/logic/doomed/DoomedRuntime.cs` rebuilds the Doomed Realms behaviour engine on betterSkillys:
  - It covers its composites (If, Cooldown, QueuedBehavior, keys, states), attacks, movement, spawning, events (death, hit, chat) and loot.
  - `port_doomed_monsters.py` copies the 163 monster definitions from Doomed Realms into `DoomedMonsters.cs` nearly verbatim, with object types looked up by id.
  - Elder monsters Doomed Realms gave no behaviour use their regular version's behaviour, which spawns the Elder versions of its minions (e.g. Elder Lord Ruthven). Where only betterSkillys has a behaviour for the regular version, the Elder one shares it (`DoomedAliases`, applied in `BehaviorDb.Initialize`).
  - Loot becomes betterSkillys drops with the same chances; soulbound bags need the same damage, as a share of the monster's HP.
  - Objects whose remote sprite no longer exists borrow the sprite of a betterSkillys object with the same name (e.g. Oryx the Mad God 5), or of `SPRITE_DONORS` in `port_doomed.py` (the Elder Beach portal uses the Beachzone portal's).
- **Economy:** `port_doomed_economy.py` generates `core/commands/doomed/DoomedEconomyData.cs` from Doomed Realms' forge recipes, sell prices and shop lists; the commands are in `DoomedEconomyCommands.cs`.
  - `/forge <slot> <slot> ...` (inventory slots 1-8, backpack 9-16), then `/yes` or `/no`. 256 recipes. Wearing a Forge Amulet halves the price and unlocks the 3 advanced recipes, and the amulet breaks when used. `/forgelist` shows what you can make, `/forgelist <item>` searches, `/forgelist all` lists everything.
  - `/sellfame <slot> ...` sells items for fame (Doomed Realms' `/sell`; betterSkillys' `/sell` is its market).
  - `/shop <name>` opens a shop whose merchants sell one list: abilities, weapons, armor, rings, consumables, dyes, keys and pets for fame, and Doomed Realms' donor shops for gold. With no name it lists the shops. Doomed Realms' default shop sold five starter weapons; here each of its store lists is a shop. Its pet generators had no price; they cost 1250 fame like betterSkillys' own.
  - `/arenas` lists running arenas, `/leaderboard` the best arena waves, `/gleaderboard` the guilds with the most fame.
  - `/afk` and `/pause` toggle Paused when no monster is near (not in arenas).
  - Supporter: `/gear` equips the best item of each of your class's slots. Admin: `/set` (the same), `/giveeff <player> <effect>`, `/message <title> <text>`, `/killallx <name>` (kills with experience), `/grave [name] [0-8]`, `/dragons` (Admin Room), `/openarena [free]` (a Battle Arena portal for 30 seconds; betterSkillys' `/arena` is its own arena).
  - Fixed from Doomed Realms: a recipe accepted the same item twice for two different ingredients, and the forge used to take the items even if they changed between `/forge` and `/yes`.
- **Not ported:** Doomed Realms commands betterSkillys already has (`/who`, `/tell`, `/visit`, `/ban`, `/rename`, `/vault`, guild commands, ...), its multiple backpacks (`/b`, `/bp`), and its group/solo joining, which it had switched off.

### betterSkillys fixes made for the port

- `World.RemoveFromWorld` now removes every Enemy from the enemy list, as `AddToWorld` adds them. Before, monsters without `<Enemy/>` (like the herding sheep) stayed in it after leaving the world.
- `NexusMerchant` returns to any `IMerchantWorld` (the nexus or the Doomed shop) instead of assuming the nexus.
- Earlier dungeon ports put map regions after Store_9 one slot off (Doomed Realms has 9 store regions, not 10); the maps that use them (Elder Undead Lair, Party Cellar, Sheep Herding, both Zombies maps) are regenerated.
