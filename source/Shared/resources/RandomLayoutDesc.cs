using System.Linq;
using System.Xml.Linq;

namespace Shared.resources
{
    // <RandomLayout> on a world: build a new layout from the rooms of its map for every instance
    public sealed class RandomLayoutDesc
    {
        // the widest passage in the map that still counts as a corridor rather than part of a room
        public readonly int Passage;
        // width of the corridors that join the rooms
        public readonly int Corridor;
        // empty tiles between two joined rooms
        public readonly int MinGap;
        public readonly int MaxGap;
        // share of the rooms on the way to the goal rooms; the rest become side rooms
        public readonly double MainPath;
        // a room holding one of these objects ends a branch (the boss room, a key room)
        public readonly string[] Goals;

        public RandomLayoutDesc(XElement e)
        {
            Passage = e.GetAttribute("passage", 2);
            Corridor = e.GetAttribute("corridor", 2);
            MinGap = e.GetAttribute("minGap", 3);
            MaxGap = e.GetAttribute("maxGap", 9);
            MainPath = e.GetAttribute("mainPath", 0.6);
            Goals = e.Elements("Goal").Select(g => g.Value.Trim()).ToArray();
        }
    }
}
