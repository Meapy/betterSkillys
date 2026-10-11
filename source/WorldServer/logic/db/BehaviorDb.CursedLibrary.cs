using Shared.resources;
using WorldServer.logic.behaviors;
using WorldServer.logic.loot;
using WorldServer.logic.transitions;

namespace WorldServer.logic
{
    partial class BehaviorDb
    {
        private _ CursedLibrary = () => Behav()
        .Init("Library Deholder",
            new State(
                new StayCloseToSpawn(0.4, 5),
                new Wander(0.2),
                new Shoot(10, count: 1, projectileIndex: 0, coolDown: 3000),
                new Shoot(0, count: 4, shootAngle: 90, projectileIndex: 0, fixedAngle: 45, coolDown: 6000, coolDownOffset: 1500)
                ),
            new ItemLoot("Health Potion", 0.05)
            )
        // keeps the library's other horrors alive
        .Init("Library Eye of the Edified",
            new State(
                new StayCloseToSpawn(0.5, 4),
                new Wander(0.4),
                new HealEntity(8, null, 300, coolDown: 3000)
                ),
            new ItemLoot("Magic Potion", 0.05)
            )
        .Init("Library Lips of the Lost",
            new State(
                new StayCloseToSpawn(0.4, 4),
                new Wander(0.3),
                new Shoot(8, count: 3, shootAngle: 20, projectileIndex: 0, coolDown: 1500),
                new Shoot(5, count: 1, projectileIndex: 1, coolDown: 4000)
                ),
            new ItemLoot("Magic Potion", 0.05)
            )
        .Init("Library Hand of the Helpless",
            new State(
                new Follow(0.5, 9, 3),
                new Wander(0.3),
                new Shoot(10, count: 1, projectileIndex: 0, coolDown: 2000),
                new Shoot(10, count: 1, projectileIndex: 1, coolDown: 2000, coolDownOffset: 150),
                new Shoot(10, count: 1, projectileIndex: 2, coolDown: 2000, coolDownOffset: 300),
                new Shoot(10, count: 1, projectileIndex: 3, coolDown: 2000, coolDownOffset: 450),
                new Shoot(0, count: 8, shootAngle: 45, projectileIndex: 6, fixedAngle: 0, coolDown: 4000, coolDownOffset: 1000)
                ),
            new ItemLoot("Health Potion", 0.05)
            )
        .Init("Library Headless Specter",
            new State(
                new Follow(0.6, 10, 2),
                new Wander(0.3),
                new Shoot(9, count: 2, shootAngle: 15, projectileIndex: 0, coolDown: 1000),
                new Shoot(9, count: 3, shootAngle: 20, projectileIndex: 1, coolDown: 2000),
                new Shoot(0, count: 6, shootAngle: 60, projectileIndex: 2, fixedAngle: 0, coolDown: 4000)
                ),
            new ItemLoot("Health Potion", 0.05)
            )
        .Init("Library Soulless Scholar",
            new State(
                new StayCloseToSpawn(0.5, 6),
                new Wander(0.4),
                new Shoot(10, count: 3, shootAngle: 15, projectileIndex: 0, coolDown: 1400)
                ),
            new ItemLoot("Magic Potion", 0.03)
            )
        .Init("Library Soulless Student",
            new State(
                new Follow(0.9, 8, 1),
                new Wander(0.4),
                new Shoot(4, count: 2, shootAngle: 20, projectileIndex: 0, coolDown: 700)
                ),
            new ItemLoot("Health Potion", 0.03)
            )
        // the bombs around Avalon's desk only fire while she fights
        .Init("Library Book Bomb",
            new State(
                new ConditionEffectBehavior(ConditionEffectIndex.Invincible, true),
                new State("Idle"),
                new State("Active",
                    new EntityNotExistsTransition("Library Avalon the Archivist", 30, "Idle"),
                    new Shoot(0, count: 4, shootAngle: 90, projectileIndex: 1, fixedAngle: 0, coolDown: 4000),
                    new Shoot(0, count: 4, shootAngle: 90, projectileIndex: 1, fixedAngle: 45, coolDown: 4000, coolDownOffset: 2000)
                    )
                )
            )
        .Init("Library Avalon Candle Helper",
            new State(
                new Shoot(0, count: 6, shootAngle: 60, projectileIndex: 0, fixedAngle: 0, coolDown: 2000),
                new Decay(8000)
                )
            )
        .Init("Library Avalon the Archivist",
            new State(
                new ScaleHP2(20),
                new DropPortalOnDeath("Realm Portal", 1.0),
                new State("Idle",
                    new ConditionEffectBehavior(ConditionEffectIndex.Invulnerable),
                    new PlayerWithinTransition(9, "Wake")
                    ),
                new State("Wake",
                    new ConditionEffectBehavior(ConditionEffectIndex.Invulnerable),
                    new Taunt("Silence in the library!"),
                    new TimedTransition(2000, "Lecture")
                    ),
                new State("Lecture",
                    new Order(20, "Library Book Bomb", "Active"),
                    new StayCloseToSpawn(0.4, 5),
                    new Wander(0.3),
                    new Shoot(12, count: 3, shootAngle: 20, projectileIndex: 2, coolDown: 1500),
                    new Shoot(12, count: 1, projectileIndex: 1, predictive: 0.3, coolDown: 2500),
                    new HpLessTransition(0.6, "Candles")
                    ),
                new State("Candles",
                    new Order(20, "Library Book Bomb", "Active"),
                    new Taunt("Let the candles light your way... out!"),
                    new StayCloseToSpawn(0.4, 5),
                    new Wander(0.3),
                    new TossObject("Library Avalon Candle Helper", 5, coolDown: 3000),
                    new Shoot(10, count: 3, shootAngle: 30, projectileIndex: 3, coolDown: 1200),
                    new Shoot(0, count: 8, shootAngle: 45, projectileIndex: 4, fixedAngle: 0, coolDown: 3000),
                    new HpLessTransition(0.3, "Final Chapter")
                    ),
                new State("Final Chapter",
                    new Order(20, "Library Book Bomb", "Active"),
                    new Taunt("Your story ends here!"),
                    new Flash(0x9933FF, 0.5, 6),
                    new Follow(0.3, 12, 3),
                    new TossObject("Library Avalon Candle Helper", 5, coolDown: 2500),
                    new Shoot(0, count: 16, shootAngle: 22.5, projectileIndex: 1, fixedAngle: 0, coolDown: 3500),
                    new Shoot(12, count: 5, shootAngle: 12, projectileIndex: 2, coolDown: 1000)
                    )
                ),
            new Threshold(0.01,
                new ItemLoot("Potion of Attack", 1),
                new ItemLoot("Potion of Wisdom", 0.5),
                new ItemLoot("Mark of the Archivist", 0.02),
                new ItemLoot("Scholar's Seal", 0.01)
                )
            );
    }
}
