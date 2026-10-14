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
| Graveyard (an account's dead characters) | `Graveyard` | To do (the server lists none) |
| Choose a name (accounts without one) | `ChooseNameRegisterDialog`, `/account/setName` | Dropped: the server answers "Unknown Error"; names come from registering |
| Credits screen, news | `CreditsScreen`, `/app/globalNews` | Dropped |
| Map editor | `NewEditorScreen` | Dropped |

## In the game

| Feature | AIR | Status |
|---|---|---|
| World, walls, objects, ground, 3D | `Map`, `Square`, objects | Done |
| Particles and effects (`ShowEffect`, auras, hit sparks) | `particles/*` | To do |
| Area attacks (`Aoe`) | `onAoe` | To do |
| Shooting, abilities, hits | `Player`, `Projectile` | Done |
| HUD: minimap, bars, equipment, inventory, backpack, stats, potions | `HUDView` | Done |
| Minimap zoom, player tooltips on the minimap | `MiniMap` | To do |
| Item tooltips | `EquipmentToolTip` | Done |
| Items: swap, drop, use, equip, bags | `ItemGridMediator` | Done |
| Chat, slash commands, speech balloons | `TextBox`, `onText` | Done |
| Tells: reply key, tab completion | `TextBox` | To do |
| Notifications over objects, global notifications (keys) | `onNotification`, `onGlobalNotification` | Done |
| Quest arrow | `QuestObjId`, `QuestArrow` | Done |
| Boss health bar | `BossHealthBar` | Done (without the damage-dealt percentage) |
| Dungeon keys view | `KeysView` | Done |
| Legendary and mythical drop pop-ups | `LegendaryNotifView` | To do |
| Portals, Nexus, Escape menu | `PortalPanel`, `onReconnect` | Done |
| Player menu: trade, invite, teleport, lock, ignore | `PlayerMenu`, `EditAccountList` | To do |
| Trading | `TradePanel`, `onTrade*` | To do |
| Party: invite, panel | `PartyPanel`, `onInvitedToParty` | To do |
| Guild: invite, register, board, chronicle, hall portal | `Guild*Panel`, `onInvitedToGuild`, `/guild/*` | To do |
| Merchants and shops (and their balloons) | `SellableObjectPanel`, `Merchant`, `onBuyResult` | Done |
| Market | `MarketPanel`, `onMarket*Result` | To do |
| Forge | `ForgePanel` | To do |
| Potion storage | `PotionsPanel`, `/account/grabPotions` | To do |
| Skin changer, reskin vendor | `CharacterChangerPanel`, `ReskinPanel` | To do |
| Name changer | `NameChangerPanel`, `onNameResult` | To do |
| Stat NPC, bounty board, money changer | `StatNPCPanel`, `BountyBoardPanel`, `MoneyChangerPanel` | To do |
| Arena wave warning | `onImminentArenaWave` | To do |
| Pictures from the server (`Pic`) | `onPic` | To do |
| Options: graphics, sound, controls, key bindings | `Options` | To do |
| Sound effects and music | `SoundEffectLibrary`, `onPlaySound`, `onSwitchMusic` | To do |
| Emotes | `Emotes` | To do |
| Death: back to the character list with the summary | `onDeath` | Done |
