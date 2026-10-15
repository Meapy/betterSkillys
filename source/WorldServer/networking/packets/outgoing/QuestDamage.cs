using Shared;

namespace WorldServer.networking.packets.outgoing
{
    // The quest's top damage dealers (name and damage, most first) and the player's own damage,
    // for clients that asked (QUEST_DAMAGE_SUBSCRIBE).
    public class QuestDamage : OutgoingMessage
    {
        public int ObjectId { get; set; }
        public (string Name, int Damage)[] Top { get; set; }
        public int Mine { get; set; }

        public override MessageId MessageId => MessageId.QUEST_DAMAGE;

        public override void Write(NetworkWriter wtr)
        {
            wtr.Write(ObjectId);
            wtr.Write((byte)Top.Length);
            foreach (var (name, damage) in Top)
            {
                wtr.WriteUTF16(name);
                wtr.Write(damage);
            }
            wtr.Write(Mine);
        }
    }
}
