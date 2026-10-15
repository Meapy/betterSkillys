using Shared;
using WorldServer.core.worlds;
using WorldServer.networking;

namespace WorldServer.core.net.handlers
{
    // The client wants its quest's top damage dealers (QuestDamage messages) from now on.
    public class QuestDamageSubscribeHandler : IMessageHandler
    {
        public override MessageId MessageId => MessageId.QUEST_DAMAGE_SUBSCRIBE;

        public override void Handle(Client client, NetworkReader rdr, ref TickTime time)
        {
            if (client.Player != null)
                client.Player.WantsQuestDamage = true;
        }
    }
}
