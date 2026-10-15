# Parity with the AIR client

Everything the AIR client (`client/src`) lets a player do, and where the Rust client stands. The
Rust client replaces the AIR one only when nothing here is left at "To do" (or knowingly dropped).
Update the status when a feature lands.

Sources: the screens (`com/company/assembleegameclient/screens`, `kabam/rotmg/ui/view`), the
server messages it handles (`GameServerConnection.mapMessages`), the account server calls it makes
(`sendRequest`), and the panels of interactive objects (`getPanel`).

## Account and screens

| Feature | AIR | Status |
|---|---|---|
| Sign in, remember the login | `AccountScreen`, `WebLoginDialog` | Done |
| Register | `/account/register` | Done |
| Change password | `/account/changePassword` | Done |
| Forgot password / recovery | `/account/forgotPassword` | Dropped: the server has no mail |
| Character list, play, delete | `CharacterSelectionAndNewsScreen` | Done |
| Buy a character slot (gold or fame) | `BuyCharacterRect`, `/account/purchaseCharSlot` | Done |
| New character, locked classes | `NewCharacterScreen` | Done |
| Skins: pick one for a new character, buy skins | `CharacterSkinView`, `/account/purchaseSkin` | Done |
| Class unlock purchase | `/char/purchaseClassUnlock` | Dropped: the server answers "Invalid Endpoint" |
| Server choice | `ServersScreen` | Done |
| Legends (fame list: week, month, all time) | `LegendsView`, `/fame/list` | Done |
| Death summary (fame breakdown) | `FameView`, `/char/fame` | Done |
| Graveyard (an account's dead characters, each opening its fame summary) | `Graveyard`, `<News>` | Done |
| Choose a name (accounts without one) | `ChooseNameRegisterDialog`, `/account/setName` | Dropped: the server answers "Unknown Error"; names come from registering |
| Credits screen, news | `CreditsScreen`, `/app/globalNews` | Dropped |
| Map editor | `NewEditorScreen` | Dropped |

## In the game

| Feature | AIR | Status |
|---|---|---|
| World, walls, objects, ground, 3D | `Map`, `Square`, objects | Done |
| Particles and effects (`ShowEffect`, hit sparks, death bursts, flashes, camera shake) | `particles/*` | Done (objects' own `<Effect>` particles, like fountains and vents, to do) |
| Area attacks (`Aoe`) | `onAoe` | Done (the server applies the damage; the client draws the blast) |
| Shooting, abilities, hits | `Player`, `Projectile` | Done |
| HUD: minimap, bars, equipment, inventory, backpack, stats, potions | `HUDView` | Done |
| Minimap zoom (buttons, - and =), player tooltips and menus on the minimap | `MiniMap` | Done |
| Item tooltips | `EquipmentToolTip` | Done |
| Items: swap, drop, use, equip, bags | `ItemGridMediator` | Done |
| Chat, slash commands, speech balloons | `TextBox`, `onText` | Done |
| Tells: Tab cycles who to whisper; G guild and P party chat; Page Up/Down history | `TextBox` | Done |
| Notifications over objects, global notifications (keys) | `onNotification`, `onGlobalNotification` | Done |
| Quest arrow | `QuestObjId`, `QuestArrow` | Done |
| Boss health bar, with the share of damage we dealt | `BossHealthBar`, `DamageCounter` | Done |
| Dungeon keys view | `KeysView` | Done |
| Legendary and mythical drop pop-ups | `LegendaryPopUpSignal` | Dropped: the AIR client never sends these signals |
| Portals, Nexus, Escape menu | `PortalPanel`, `onReconnect` | Done |
| Player menu: trade, invite, teleport, whisper, lock, ignore; nearby players | `PlayerMenu`, `PartyPanel`, `EditAccountList` | Done (opened from the nearby list, the minimap and names in the chat) |
| Trading | `TradePanel`, `onTrade*` | Done |
| Party: invite, join | `PartyPanel`, `onInvitedToParty` | Done |
| Guild: invite and join | `GuildInvitePanel`, `onInvitedToGuild` | Done |
| Guild: create, renounce, board (read and write), member list (promote, demote, remove), hall portal | `Guild*Panel`, `/guild/*` | Done |
| Merchants and shops (and their balloons) | `SellableObjectPanel`, `Merchant`, `onBuyResult` | Done |
| Market | `MarketPanel`, `onMarket*Result` | To do |
| Forge | `ForgePanel` | Dropped: no map places a Forge (the server's `/forge` works from chat) |
| Potion storage: store, take, drink, drink to the maximum, upgrade | `PotionsPanel`, `/account/grabPotions` | Done |
| Character changer, the Armoire (skins) | `CharacterChangerPanel`, `ReskinPanel` | Done |
| Name changer | `NameChangerPanel`, `onNameResult` | Done |
| Stat NPC, bounty board, money changer | `StatNPCPanel`, `BountyBoardPanel`, `MoneyChangerPanel` | Dropped: no map places them |
| Arena wave and clock | `onImminentArenaWave`, `ArenaMenu` | Done |
| Pictures from the server (`Pic`) | `onPic` | Dropped: the server never sends `Pic` |
| Options: graphics, sound, controls, key bindings | `Options` | To do |
| Sound effects and music | `SoundEffectLibrary`, `onPlaySound`, `onSwitchMusic` | To do |
| Emotes (in chat and speech balloons, animated ones too) | `Emotes`, `EmoteGraphicHelper` | Done |
| Death: back to the character list with the summary | `onDeath` | Done |
