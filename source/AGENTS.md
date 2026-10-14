# Server development

C# on .NET 8: `betterSkillys.sln` has the account server (`App`), the world server
(`WorldServer`), the shared library (`Shared`) and Redis. It's based on TK-Reborn. Read
[../AGENTS.md](../AGENTS.md) first for the shared rules, above all: **ask before restarting the
live servers**, because players get disconnected.

## Projects

| Project | What it is | Config |
|---|---|---|
| `App` | Account server (ASP.NET): login, character list, server list, web endpoints in `Controllers/` | `App/server.json` (Redis, port 8080) |
| `WorldServer` | The game: worlds, players, monsters, commands, networking | `WorldServer/wServer.json` (Redis, port 2051, public address, realm settings) |
| `Shared` | Game data (`resources/`), the Redis database layer (`database/`), shared types | |
| `Redis` | Bundled Redis 3.2 for Windows, `redis.local.conf` | |

## Build and check

```
dotnet build source\betterSkillys.sln -c Debug          # what Run Server.bat does
dotnet build source\WorldServer\WorldServer.csproj -c Debug -o build\wscheck   # while the live server runs
```

- **The live servers run from `*/bin/Debug/net8.0`**, and their files are locked while they run.
  To check that something compiles without stopping them, build into another folder with `-o`,
  as above.
- **`Optimize` is on for `WorldServer` and `Shared` in Debug too**, because Debug is what
  `Run Server.bat` builds and runs.
- **To try a change in game,** use the isolated stack, `tools/testclient/iso_stack.py start`. It
  builds App and WorldServer from the current source into `build/iso`, on Redis 6380 (a copy of the
  live data), account 8081 and world 2052. See client/AGENTS.md for the test client that connects
  to it, and stop the stack with `iso_stack.py stop` when done.
- **Logs** are in `WorldServer/bin/Debug/net8.0/logs/wServer/<date>/info.txt` (and `error.txt`), and
  `App/bin/Debug/net8.0/logs/app/`. Ticks slower than 200 ms are logged as "Slow tick".
- **Deploying a change to the live server** means rebuilding and restarting it. Ask first, and
  check that nobody is connected (the log shows connects and disconnects) or that the players are
  fine with it.

## How the world server works

- **World threads:** each world runs on a world thread (`core/worlds/RootWorldThread.cs`) at 100 ms
  ticks. The thread sleeps until a player's message arrives or the next tick is due; it doesn't
  spin. `Program.cs` asks Windows for 1 ms timer precision.
- **Sending:** messages a thread sends during one pass of its loop are queued and sent together at
  the end (`BeginBatch`/`FlushBatch` in `core/connection/NetworkHandler.cs`). A message that
  doesn't fit in the send buffer goes first in the next send, so the order is kept.
- **Incoming messages:** ids are the `MessageId` enum in `networking/packets/MessageIds.cs`.
  Handlers are classes deriving from `IMessageHandler` in `core/net/handlers/`, found by
  reflection, so a new handler needs no registration (see `ViewRadiusHandler.cs`). An unknown id
  is logged and counted as packet spam, and more than 32 disconnect the player. The client's ids
  are in its `GameServerConnection.as`, so keep the two in step and deploy them together.
- **Players:** `Player` is split into partial classes in `core/objects/player/`
  (`Player.Update.cs`, `Player.UseItem.cs`, and so on).
- **What a player sees:** `Player.SightRadius`, 15 tiles by default and up to 40 when the client
  asks (message 88, `SetSightRadius`). Sight shapes are cached per radius, so a bigger view costs
  only the players who use it. `Player.VISIBILITY_RADIUS` is 15 and `MAX_VISIBILITY_RADIUS` 40.
  Enemies only tick in active chunks near players.
- **Worlds:**
  - `core/worlds/WorldManager.cs` creates them. Its default case asks
    `impl/doomed/DoomedWorlds.Create` first, for the Doomed Realms minigames.
  - World definitions are in `Shared/resources/xml` (`Worlds.xml`, `Dungeons.xml`,
    `custom/DoomedRealmsWorlds.xml`). Maps are `.jm` files in `Shared/resources/worlds/`.
- **Removing entities:** `World.RemoveFromWorld` removes any `entity is Enemy` from the enemy
  list, as `AddToWorld` adds them. Don't go back to checking `ObjectDesc.Enemy`: monsters without
  `<Enemy/>` (like the herding sheep) would stay in the list.
- **Merchants:** they return to any `IMerchantWorld` (the nexus or the Doomed shop).
- **Shooting:** in `PlayerShootHandler.cs`, the rate-of-fire and projectile-count checks apply
  only to players who aren't admins.

## Behaviours (monsters)

- **betterSkillys behaviours:** `logic/db/BehaviorDb.<Area>.cs`, partial classes of `BehaviorDb`.
  The building blocks are in `logic/behaviors/` and `logic/transitions/`.
- **Doomed Realms monsters** run on a rebuilt copy of the Doomed Realms behaviour engine,
  `logic/doomed/DoomedRuntime.cs`:
  - It covers its composites (If, Cooldown, QueuedBehavior, states), attacks, movement, spawning,
    events (death, hit, chat, `WorldEvent`, `OnHit` from `Enemy.Damage`) and loot.
  - The monster definitions in `logic/doomed/DoomedMonsters.cs` are generated.
  - Elder monsters without a behaviour share their regular version's (`DoomedAliases`, applied in
    `BehaviorDb.Initialize`).
  - A hand-written behaviour replaces a generated one: `BehaviorDb.DoomedMonsters.cs` registers with
    `InitDoomed`, which steps aside for an `Init` of the same monster. `BehaviorDb.Tomb.cs` builds
    both the Tomb and the Elder Tomb this way.
- **Never spawn hostile monsters in the nexus**: they kill low-level characters there.
- **The start-up log lists "Failed to add behavior ... Xml data not found"** for a few upstream
  behaviours with no object data. That's long-standing upstream noise, not a new error.

## Commands

- **Where:** `core/commands/`, `admin/`, `player/`, `supporter/` and `doomed/`. Each command is a
  nested class of the partial `Command` class; `CommandManager` finds them by reflection.
- **Permissions:** each command overrides `RankRequirement` (`Command.cs`) with the rank it needs. The account's `admin`
  flag passes every check.
- **Name clashes:** Doomed Realms commands that clash with betterSkillys ones were renamed
  (`/sellfame` for its `/sell`, `/openarena` for its `/arena`).

## Redis data

- **Accounts:** `account.<id>` hashes (fields such as `name`, `rank`, `admin`). The `names` hash
  maps upper-case names to ids, and characters are `char.<accountId>.<charId>`.
- **Field formats:** numbers are stored as strings, item lists as little-endian ushort arrays, and
  the `admin` flag as a single byte 0x01.
- **Ranks:** 0 regular to 6 admin. A rank change applies from the player's next login.
- **Arena leaderboard:** the sorted set `doomed.arenaLeaderboard`.

```
source\Redis\redis-cli.exe hget names MEAPY
source\Redis\redis-cli.exe -p 6380 hgetall account.1     # isolated test copy
```

- **Accounts in use:**
  - Meapy (account 2) is the owner's account. Don't change it or its characters without asking.
  - Tester (account 1) is the throwaway test account: change its characters freely, preferably on
    the isolated copy (port 6380).

## Doomed Realms port (generated files)

`python tools\port_doomed.py --dr "<Doomed Realms repo>"` regenerates the Doomed Realms content
and calls the other `tools/port_doomed_*.py` scripts. Its outputs are:
- **XML:** `Shared/resources/xml/custom/DoomedRealms.xml` and `DoomedRealmsGrounds.xml`, plus the
  client's copies.
- **Worlds:** `Shared/resources/xml/custom/DoomedRealmsWorlds.xml` and `Shared/resources/worlds/Doomed/*.jm`
  (`port_doomed_worlds.py`).
- **Monsters:** `WorldServer/logic/doomed/DoomedMonsters.cs` and
  `logic/db/BehaviorDb.DoomedMonsters.cs` (`port_doomed_monsters.py`).
- **Allies and pets:** `logic/db/BehaviorDb.DoomedAllies.cs` and `BehaviorDb.DoomedPets.cs`.
- **Economy:** `core/commands/doomed/DoomedEconomyData.cs` (`port_doomed_economy.py`).

Fix problems in the scripts and regenerate; don't edit these files by hand. Type codes 0xD000 to
0xDFFF are kept stable in `tools/doomed/types.json`, because characters store items by type code:
never renumber them.

Behaviour that isn't generated: `DoomedRuntime.cs`, `core/worlds/impl/doomed/DoomedWorlds.cs` (arenas, zombies,
herding, shop), `DoomedRealmsCommands.cs`, `DoomedEconomyCommands.cs`, `Player.UseItem.Doomed.cs`
(class abilities) and `Halo.cs`. Doomed Realms' slot types 26 and 28 are 126 and 128 here.
