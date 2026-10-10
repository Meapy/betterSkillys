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
- **Dungeons:** 26 Doomed Realms worlds that have maps, generated by the same tool:
  - 8 Elder dungeons (Pirate Cave, Draconis, Abyss, Snake Pit, Spider Den, Sprite World, Tomb, Undead Lair).
  - Doomed Cellar, Hash Cellar, Doom Crystal, Forgotten Island, Zombies, Zombies Minigame, Sheep Herding, Forest Sanctuary, Christmas Oryx's Chamber, Party Cellar, Kitchen, Secret, Battle Arena, Free Battle Arena, Underground Mine, The Gauntlet, Turkey Hunting Grounds and Admin Room.
  - `port_doomed_worlds.py` converts the `.wmap` maps to `.jm` in `source/Shared/resources/worlds/Doomed/` and writes `xml/custom/DoomedRealmsWorlds.xml`.
  - Their portals point at those worlds and their keys create the portals (CreatePortal). Monsters' death portals work as in Doomed Realms.
  - Admins can open any dungeon with `/dungeon <name>`; with no name it lists the Doomed Realms ones. Several of them (Battle Arena, Kitchen, Mine, Gauntlet, ...) have no other entrance.
  - The Elder Manor, Ocean Trench, Jungle, Beach and Davy keys stay out: Doomed Realms never made those dungeons.
- **Monsters:** `source/WorldServer/logic/doomed/DoomedRuntime.cs` rebuilds the Doomed Realms behaviour engine on betterSkillys:
  - It covers its composites (If, Cooldown, QueuedBehavior, keys, states), attacks, movement, spawning, events and loot.
  - `port_doomed_monsters.py` copies the 121 monster definitions from Doomed Realms into `DoomedMonsters.cs` nearly verbatim, with object types looked up by id.
  - Loot becomes betterSkillys drops with the same chances; soulbound bags need the same damage, as a share of the monster's HP.
  - Objects whose remote sprite no longer exists borrow the sprite of a betterSkillys object with the same name (e.g. Oryx the Mad God 5).
- **Not ported yet:** commands that depend on Doomed Realms systems (forge, shop, arenas, leaderboards).
