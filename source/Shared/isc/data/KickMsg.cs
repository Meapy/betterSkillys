namespace Shared.isc.data
{
    // Asks the other world servers to sign this account out: it is signing in on the sender.
    public struct KickMsg
    {
        public int AccountId;
    }
}
