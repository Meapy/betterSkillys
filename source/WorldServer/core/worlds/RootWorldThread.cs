using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Threading;
using System.Threading.Tasks;
using WorldServer.core.connection;
using WorldServer.utils;

namespace WorldServer.core.worlds
{
    public sealed class RootWorldThread
    {
        public const int TICK_TIME_MS = 100; // set this to 200 for 5tps 50 for 20 and 100 for 10 tps, never touch the deltatime value 

        private readonly WorldManager WorldManager;
        private World World;
        private bool Stopped;

        public RootWorldThread(WorldManager worldManager, World world)
        {
            WorldManager = worldManager;
            World = world;

            Run();
        }

        private void Run()
        {
            Task.Factory.StartNew(() =>
            {
                var watch = Stopwatch.StartNew();

                var sleep = TICK_TIME_MS;

                var lastMS = 0L;
                var mre = new ManualResetEvent(false);

                var realmTime = new TickTime();

                var elapsed = 0L;

                // messages sent by this thread go out together at the end of each pass
                NetworkSendHandler.BeginBatch();
                while (!Stopped)
                {
                    World.ProcessPlayerIO(ref realmTime);

                    var currentMS = realmTime.TotalElapsedMs = watch.ElapsedMilliseconds;
                    var dt = currentMS - lastMS;
                    lastMS = currentMS;

                    elapsed += dt;
                    if(elapsed >= TICK_TIME_MS)
                    {
                        realmTime.TickCount++;
                        realmTime.ElapsedMsDelta = (int)elapsed;

                        var tickStart = watch.ElapsedMilliseconds;
                        try
                        {
                            if (World.Update(ref realmTime))
                            {
                                Stopped = true;
                                break;
                            }
                        }
                        catch (Exception e)
                        {
                            Console.WriteLine($"[{World.IdName} {World.Id}] Tick: {e.StackTrace}");
                        }

                        // a slow tick delays every player in this world
                        var tickMs = watch.ElapsedMilliseconds - tickStart;
                        if (tickMs > 200)
                            StaticLogger.Instance.Warn($"[{World.IdName} {World.Id}] Slow tick: {tickMs} ms, {World.Players.Count} players");

                        elapsed = 0;
                    }

                    NetworkSendHandler.FlushBatch();

                    if (World.Players.Count == 0)
                        Thread.Sleep(TICK_TIME_MS);
                    else
                    {
                        // sleep until a player's message arrives or the next tick is due
                        var untilTick = TICK_TIME_MS - elapsed - (watch.ElapsedMilliseconds - lastMS);
                        if (untilTick > 0)
                            World.WaitForIO((int)untilTick);
                    }
                }

                NetworkSendHandler.FlushBatch();
                WorldManager.RemoveWorld(World);

            }, TaskCreationOptions.LongRunning);
        }

        public void Stop()
        {
            if (Stopped)
                return;
            Stopped = true;
        }
    }
}
