# Changelog

What changed in the game, in plain words for players: the client, the servers and the content.
The game shows this in its Changelog window. Newest first.

Every change goes under `## Unreleased` at the top as soon as it lands. Publishing a client
version (`python tools/build_client_rs.py --publish`) needs a section for that version and
refuses while an `Unreleased` one is left, so rename it to `## <version> (<date>)` then. The
version number itself is in `client-rs/Cargo.toml`.

## 1.1.6 (2026-10-16)
- Server updates and the 2-hourly restart no longer kick anyone. When an update is waiting, a
  chat message says so: you keep playing where you are (dungeons included) and switch to the
  new version the next time you go to the Nexus, or after an hour at the latest.
- The Wine Cellar Key no longer opens the Wine Cellar from anywhere: like a Wine Cellar
  Incantation, it unlocks the portal Oryx 1 leaves behind. The Nexus Bazaar now sells the Wine
  Cellar Incantation (125 fame) instead of the key.
- Using an item where it can't be used (an incantation away from its locked portal, anything
  without enough mana or while trading) no longer makes it vanish from your inventory: it stays
  where it was, and a potion goes back on its stack.

## 1.1.5 (2026-10-16)
- Cloth and dye items (and a few dungeon objects) show their real names instead of codes like "{textiles.Small_Vine_Cloth}".
- The fight against Oryx now goes all the way: when a realm closes you can follow him from his
  Castle through his Chamber (Oryx 1), his Wine Cellar (Oryx 2), the new Oryx's Sanctuary
  (Oryx 3), the Hash Cellar (Oryx 4) and finally the Doomed Cellar (Oryx 5). Each Oryx opens
  the way to the next, and each kill is announced to the whole server.
- Oryx's Sanctuary is a new dungeon: fight through his nobles and his court until the Sanctum's
  gate opens, then face Oryx 3. Get far away when he shouts "KNEEL!", and kill his three
  Protectors at half health.
- Every stage is harder than the one before, with more health, defense and damage, and better
  loot: Oryx 4 and 5 can drop the Oryx set (Ring of Oryx, the Oryx armours, Oryx Tooth Sword
  and more).
- Oryx 1 no longer gets stuck in his second dance, Oryx 2 calls his guards at half health, and
  Oryx 4 and 5 have new fights. Oryx 5 can't be hurt until his Trees of Doom are cut down.
- The Stone Guardians in Oryx's Castle now use their ring attacks (they never fired before), but
  have less health and defense.
- In Oryx's dungeons the quest arrow points at Oryx, and the portals he drops sit beside his loot
  instead of under it. The Realm Portal after each Oryx stays open for two minutes.
- Oryx's loot was rebalanced to rise with each stage: more stat potions, Life and Mana and
  higher-tier gear the further you go. Potion of Oryx and the Oryx set are now real rarities
  (Oryx 3 to 5), and Oryx 3 can drop Oryx's Escutcheon.
- The Oryx set, Doom Poison and Sword of Dark Enchantment now drop in white bags and are announced
  to everyone; Potion of Oryx and Potion of Power drop in the special bag.
- Each Oryx now leaves a locked portal behind. To follow him, someone has to use the right
  incantation beside it: a Wine Cellar Incantation after Oryx 1, an Oryx's Sanctuary Incantation
  after Oryx 2, a Hash Cellar Incantation after Oryx 3 and a Doomed Cellar Incantation after
  Oryx 4. Incantations drop from the stage before (and the Wine Cellar one from several dungeon
  bosses), and the Nexus Bazaar sells the last three. The Hash and Doomed Cellar Incantations no
  longer open their dungeons from anywhere.
- The 30% weekend loot boost only applies on weekends now. It was quietly applied every day,
  so drops on weekdays are a little lower than before.

## 1.1.4 (2026-10-15)
- Assassin poisons work properly: they ignore the monster's defense, deal their full damage over
  the time they list, and can finish a monster off. Before, a poisoned monster could be left
  alive on 0 health. Other abilities that deal damage can now finish monsters off too.
- Poison tooltips show how much damage the poison does, over how long, how wide it spreads and
  what it deals on impact.
- Pets that fired projectiles in Doomed Realms (Kythal, Cookiee, Bess, Nutt, Sphinx, Twilight
  Sparkle and others) shoot their real projectiles again, with their own spreads and rings,
  instead of a thin red line. Their hits count as yours for loot, experience and quests.
- Pet generator tooltips show what the pet does: its damage per second on one enemy and how
  much HP and MP it restores and how often.
- The shared vault chests in the nexus keep what's in them when the server restarts.

## 1.1.3.1 (2026-10-15)
- In the game, your gold and fame show at the top right of the screen again, with the frame
  rate under them.

## 1.1.3 (2026-10-15)
- Walking onto a square you can't stand on (it happened in the Sprite World) no longer
  disconnects you: the server puts you back where you were.
- A Servers screen again: the Servers button on the character screen (or the "Server:" pill under
  your characters) lists the world servers and how full each is. Click one to play on it.

## 1.1.2 (2026-10-15)
- The character list is more compact, fitting up to four characters a row, and scrolls with the
  mouse wheel when you have more than fit on the screen.
- Your fame and gold are back in the top right corner of the character screen.

## 1.1.1 (2026-10-15)
- Rank stars are back. The character screen shows your rank (hover over it to see which colour
  each range of stars earns) and, on each character, its progress toward the next class quest.
- Choosing a class shows its five class quests and how many you have left in total; a class's
  page shows your best level and fame with it and the fame for its next quest.
- In the Nexus and other safe maps your rank shows at the top left, chat messages show the
  speaker's star, and hovering over a nearby player shows their rank.

## 1.1.0 (2026-10-15)
- The game now keeps itself up to date: new versions download in the background while you play and
  switch over the next time you take a portal. No more zips to download.
- A Changelog window (on the character screen, in the Escape menu, or click the version number)
  shows what's new.
- Item tooltips compare with what you're wearing: better numbers in green, worse in red.
- The chat box edits like a normal text box: arrow keys, Home and End, Shift to select, Ctrl+A,
  Ctrl+C, Ctrl+X, Ctrl+V, and Ctrl+Z to undo.
- Under a quest's health bar: the top 3 damage dealers and your own share.
- Status effects show on players and enemies, with sharper icons and their names rising when
  you gain one; drunk, blind, paused and others change the view as they should.
- Other players' names, coloured for your party and guild, with a star for their rank.
- Hover over a nearby player for their details.
- Potions stack up to 6 in the HP and MP slots; drop or double-click a potion to add it.
- The Nexus fountains heal you when you stand by them.
- The realm's name and player count show over the minimap; the mouse wheel zooms the minimap.
- World zoom between 90% and 130%, a frame rate limit and an unlocked frame rate, and a much
  faster game (often over 1000 frames a second).
- Return home goes back to the main menu; settings and your login are kept between versions.
- The delete button on character cards shows its X again.

## 1.0.0 (2026-10-15)
- The first version of the new client for Windows: the whole game, from signing in to trading,
  guilds, the marketplace, sound and options.
