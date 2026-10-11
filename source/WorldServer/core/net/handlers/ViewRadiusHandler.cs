using Shared;
using WorldServer.core.worlds;
using WorldServer.networking;

namespace WorldServer.core.net.handlers
{
    // The client's view reaches this far (in tiles): send it tiles and objects that far out.
    public class ViewRadiusHandler : IMessageHandler
    {
        public override MessageId MessageId => MessageId.VIEW_RADIUS;

        public override void Handle(Client client, NetworkReader rdr, ref TickTime time)
        {
            var radius = rdr.ReadInt32();
            client.Player?.SetSightRadius(radius);
        }
    }
}
