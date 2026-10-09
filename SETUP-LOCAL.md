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

## Changes from upstream

- `client/src/WebMain.as`: on a first run the map scale setting (`mscale`) is undefined until the player enters the game. The first window resize divided by it and asked for a 0-pixel-wide Stage3D back buffer (Error #3780); it now defaults to 1.
- `source/App`: `char/list` and `app/serverList` send 127.0.0.1 as the world server address to clients on this PC.
- `wServer.json`: port 2051 instead of 2050, and the public address.
