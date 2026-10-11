using WorldServer.logic.behaviors;
using WorldServer.logic.loot;
using WorldServer.logic.transitions;

namespace WorldServer.logic
{
    partial class BehaviorDb
    {
        // the Beachzone host has no attacks: it dances around the party until it's beaten
        private _ Beachzone = () => Behav()
        .Init("Masked Party God",
            new State(
                new ScaleHP2(20),
                new State("Party",
                    new StayCloseToSpawn(0.4, 6),
                    new Wander(0.4),
                    new Taunt(0.5, new Cooldown(10000, 0), "Party at the beach!", "Who brought the snacks?", "Surf's up!"),
                    new HpLessTransition(0.25, "Tired")
                    ),
                new State("Tired",
                    new Taunt("Okay, okay, the party's over..."),
                    new StayCloseToSpawn(0.2, 3)
                    )
                ),
            new Threshold(0.01,
                new ItemLoot("Potion of Speed", 1),
                new ItemLoot("Potion of Dexterity", 0.5)
                )
            );
    }
}
