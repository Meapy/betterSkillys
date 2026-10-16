# Client development (deprecated)

**This client is deprecated.** Players use the Rust client ([../client-rs/AGENTS.md](../client-rs/AGENTS.md)),
and the live servers are on the VPS (dodmg.com), not the `188.141.12.3` address below. Don't build,
test or change this client unless the owner asks; for in-game testing use the Rust client's
scripted flags. Its `src/` is still where the Rust client reads its art and data from
(`tools/gen_rs_manifest.py`), so leave those files in place. The notes below are kept for
reference.

The client is an Adobe AIR desktop app written in ActionScript 3 (`src/`). It is decompiled 7.0
client code, so many files use names like `_arg_1` and `_local_2`. Match the style of the file
you're in. It uses Robotlegs/Swiftsuspenders (`[Inject]`, `*Mediator`, `*Config` classes) and
as3-signals. Read [../AGENTS.md](../AGENTS.md) first for the shared rules.

## Build and run

```
python tools\build_client.py                           # client\bin\client.swf, account server localhost:8080
python tools\build_client.py 188.141.12.3 --package    # dist\betterSkillys-Client (+ .zip) for players
Play Client.bat                                        # runs client\bin\client.swf with adl
```

- **Account server address:** it's compiled into `kabam/rotmg/application/impl/LocalhostSetup.as`.
  `build_client.py` copies the sources to `build/src` and sets the address there, so never edit the
  host into `src/`.
- **Packaging:** `--package` builds into `dist/betterSkillys-Client.new`, then swaps it in. If the
  old folder is in use (someone is playing from it), it keeps `.new` and says so. Never delete or
  overwrite `dist/betterSkillys-Client` by hand while it may be running: a half-replaced AIR runtime
  stops the app from starting.
- **Optimised builds:** the normal builds are release builds (`-optimize=true`, no debug info). A
  debug build runs noticeably slower, so never use one for timing.
- **App descriptor:** `WebMain-app.xml`, with `renderMode` direct and the GPU (Stage3D) renderer.

## How a frame is drawn

`GameSprite.onEnterFrame` → `updateFrame`:
1. **Update:** `map.update` (objects, projectiles, particles) and `camera_.update`.
2. **Draw:** `map.draw(camera)` (`com/company/assembleegameclient/map/Map.as`):
   - **Ground:** static tiles are copied into ground chunks (one 128×128 image per 16×16 tiles, at
     most 2 rebuilt per frame, freed after 10 s unseen). Tiles that can't be chunked (animated
     tiles, tiles with blended edges) are drawn in two passes, all base faces then all overlays.
     Only tiles on screen, plus a margin, are drawn (`Map.culling`). `Square` and `SquareFace` hold
     the tile faces.
   - **Objects** (`GameObject.draw` and subclasses) push `IGraphicsData`: `GraphicsBitmapFill`
     quads, `GraphicsGradientFill` shadows, and 3D objects through `Object3DStage3D`.
   - **Renderer:** `Renderer.renderScene` (`kabam/rotmg/stage3D/Renderer.as`) turns that list into
     GPU draws.
3. **HUD:** the HUD, minimap and boss bar are Flash display objects, updated through signals
   (`drawCharacterWindow`, `updateHUD`).

`Renderer.prepareQuads` puts every plain quad of the frame into one vertex buffer (5 floats per
vertex), uploaded once:
- **Atlas:** images up to 128 px that have been in use for a while are packed into up to six
  1024×1024 atlas pages (`graphic3D/TextureAtlas.as`).
- **Shadows:** they go into their own vertex buffer and are drawn in one call.
- **Draw runs:** each run of quads with the same page, or the same image and repeat, is one draw
  call. GPU state that is already bound isn't set again.

**Debug switches** turn each optimisation off for comparison: `Renderer.batching`,
`Renderer.atlasing`, `Map.chunking`, `Map.culling` and `Face3D.caching`.

**View radius:**
- **Client:** the map scale (`/mscale`, `Parameters.data_.mscale`) and the window size decide how
  far the view reaches. `Map.viewBounds()` works that out as `Map.viewRadius_`, 15 to 40 tiles.
- **Server:** `GameSprite` sends it to the server (`VIEW_RADIUS`, message 88) at most once a second
  when it changes. The server then sends tiles and objects that far.

**Software rendering:** with hardware acceleration off (`Parameters.isGpuRender()` false), `Map`
draws through Flash's vector graphics instead. Keep that path working when you change `Map`.

## Stage3D rules learned the hard way

- **Vertex buffers:** upload a vertex buffer in full once when you create it (zeros are fine).
  Drawing from a partly uploaded buffer fails with Error #3608, and without error checking the
  draw is skipped silently, so sprites just vanish.
- **Index buffers:** Stage3D checks every index in the index buffer against the size of the bound
  vertex buffers (Error #3609), not only the indices drawn. A smaller vertex buffer needs its own
  index buffer, as the shadows have.
- **GPU texture cache:** it is keyed by `BitmapData` (`TextureFactory`, `TextureAtlas`). An image
  whose pixels change must be a new `BitmapData`, or the old texture stays on screen. Free
  textures you stop using with `TextureFactory.disposeTexture`.
- **One-draw state in `GraphicsFillExtra`:** colour transform, UV offset, sink level and vertex
  buffer are stored per bitmap. `getColorTransform` *uses up* the tint (a tint lasts one draw),
  so code that only needs to look at it must call `peekColorTransform`. (The atlas check once
  used it up, and flashing objects lost their flash.)
- **Quad corners:** a quad's corners are the texture's pixels (0,0), (w,0), (0,h) and (w,h) through
  the fill's matrix, with y flipped. w and h are the image size rounded up to powers of two.
- **Error checking:** to see Stage3D errors, turn on `context3D.enableErrorChecking`. In the test
  client, that's `#errors 1`. It slows rendering a lot, so turn it off for timing.

## Performance

**Overlay:** Performance Stats (Options, or the toggle key) shows, besides FPS:
- **`MAP: x ms (GPU y), N draws`:** map draw time per frame, the part spent in `Renderer`, and the
  GPU draw calls of the last frame.
- **`tiles, objects a + b, hud | draws q c s 3d`:** where map and HUD time goes, and the draw calls
  by kind.
- **`frame = game + flash`:** time in `GameSprite`'s frame update versus the runtime's own work
  (compositing the display list over the 3D view, garbage collection).

**What limits the frame rate:**
- **Compositing floor:** AIR spends about 4 ms putting together every frame in which anything
  changes at 4K, whatever the game does. That's a ceiling of roughly 240 FPS, the same windowed
  or fullscreen, with or without the HUD. Hiding the HUD, antialiasing or a lower resolution
  barely change it.
- **Game code:** at 4K with the whole screen in view, the game takes about 8 ms: about 3 ms
  submitting GPU draws, 2.8 ms on objects and 1.4 ms on tiles.

**Rules for code that runs every frame:**
- **Allocate nothing per frame.** Avoid `new` objects, strings and arrays. Garbage costs about
  0.3 µs per object in collection.
- **Dictionary keys:** a `Number` (not int) key is turned into a string on every lookup. Key by
  object or int instead (see `TextureRedrawer`, `FaceShades`).
- **Text fields:** setting `TextField.text` or calling `SimpleText.updateMetrics()` lays text out
  again and allocates. Only do it when the value changed. The boss bar used to do it once per
  object in the world, every frame.
- **Matrices:** `Matrix.transformPoint`, `Matrix3D.rawData` and `DisplayObject.filters` getters
  allocate.

**Measure:** use the test client.
- **Timing:** a `--release` build gives the true numbers.
- **Allocations:** in a debug build, `#sample 60` writes what allocated over 60 frames, by type and
  call site, to `%APPDATA%\betterSkillysPortTest\Local Store\samples.txt`.

## Messages to the server

1. **Outgoing class:** add it in `kabam/rotmg/messaging/impl/outgoing/`, as `ViewRadius.as` is.
2. **Wire it up:** in `kabam/rotmg/messaging/impl/GameServerConnection.as`:
   - add a constant id;
   - `messages.map(ID).toMessage(Class)` in `mapMessages`, and `unmap` in `unmapMessages`;
   - a sender method (`messages.require(ID)`, then `serverConnection.sendMessage`).
3. **Server side:** use the same id in `source/WorldServer/networking/packets/MessageIds.cs` with a
   handler there (see source/AGENTS.md).

Deploy both sides together. An old server logs an unknown message id as an error and counts it
as packet spam; more than 32 of them disconnect the player.

## Content data

- **Game data:** object, item and ground XML is embedded through `kabam/rotmg/assets/EmbeddedData.as`.
  The Doomed Realms data (`assets/custom/xmls/DoomedRealms*.xml`) and sprite sheets
  (`assets/doomed`, registered with a `dr` prefix) are generated by `tools/port_doomed.py`. Don't
  edit them by hand.
- **Slot types:** Doomed Realms slot types are 126 and 128 (not 26/28, which betterSkillys uses for
  pet eggs and lutes). They are named in `kabam/rotmg/constants/ItemConstants.as`.
  `kabam/rotmg/classes/ClassesConfig.as` also loads the Doomed Realms classes.

## Test client

`tools/testclient/` has a separate test build with its own app id and window. It needs the
throwaway Tester account in `test-account.local.txt`.

```
python tools\testclient\iso_stack.py start                    # Redis 6380, account 8081, world 2052
python tools\testclient\build_test_client.py --port 8081 --release
powershell tools\testclient\testclient.ps1 start
powershell tools\testclient\testclient.ps1 front
powershell tools\testclient\testclient.ps1 enter              # Play, then the first character
powershell tools\testclient\testclient.ps1 send "#fps 1000" "/nexus"
powershell tools\testclient\testclient.ps1 max
powershell tools\testclient\testclient.ps1 shot build\test\shot.png
python tools\testclient\iso_stack.py stop
```

**The `send` hook (`TestHook.as`):**
- **Chat:** lines without `#` are sent as chat, so slash commands work too.
- **`#` switches:** `#batch|atlas|chunk|cull|cache 0|1`, `#errors 1`, `#angle <rad>`,
  `#mscale <n>`, `#full 1`, `#hud 0`, `#mapvis 0`, `#fps <n>`, `#skip 1|2|3` (skip the frame
  update / the map draw / the present), `#poke 1` (one tiny display change per frame) and
  `#sample <frames>`.
- **Errors:** uncaught errors go to `errors.txt` beside `autochat.txt`.

**Quirks:**
- **The window must be in front:** AIR stops running frames while its window is covered or the
  display is asleep. The map then never loads and the hook reads nothing.
- **Taking the screen:** bringing the window to the front takes over the screen. Check
  `testclient.ps1 idle` (ms since the last input), and ask the person at the PC before doing it
  while they're active.
- **Input:** clicks work through window messages; keystrokes don't, which is why commands go
  through the hook.
- **Screenshots:** `PrintWindow` misses the Stage3D view; `shot` copies the screen instead.
- **AS3 in the hook:** fully qualified class names inside the hook's package don't compile; import
  the classes.

## Known quirks

- **First run:** `mscale` is undefined until the player first enters the game. `WebMain` defaults
  it to 1, because a 0-wide Stage3D back buffer raises Error #3780.
- **Frame-rate setting:** the FPS option defaults to 60 (choices up to 360), so the frame rate can
  look capped.
- **Frame timer:** `GameSprite` keeps `lastUpdate_` current even when a frame throws. Otherwise
  the next frame moves the player by the whole time since the error.
