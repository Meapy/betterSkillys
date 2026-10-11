using WorldServer.core.objects;
using WorldServer.core.worlds;

namespace WorldServer.logic.transitions
{
    internal class OnParentDeathTransition : Transition
    {
        // State storage: one ParentWatch per host. The transition object is shared by every
        // monster using this behaviour, so the flag can't live on the transition itself.
        private sealed class ParentWatch
        {
            public bool ParentDead;
        }

        public OnParentDeathTransition(string targetState)
            : base(targetState)
        {
        }

        protected override bool TickCore(Entity host, TickTime time, ref object state)
        {
            if (state is not ParentWatch watch)
            {
                watch = new ParentWatch();
                state = watch;
                if (host is Enemy enemyHost && enemyHost.ParentEntity != null)
                    enemyHost.ParentEntity.OnDeath += (sender, e) => watch.ParentDead = true;
            }

            return watch.ParentDead;
        }
    }
}
