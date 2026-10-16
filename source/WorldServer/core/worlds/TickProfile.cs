using System;
using System.Diagnostics;
using System.Text;

namespace WorldServer.core.worlds
{
    /// <summary>
    /// Where a world's ticks spend their time, summed over a period and logged as "Tick stats"
    /// by its world thread: ticks, their work and the gaps between them (what players feel as
    /// stutter), each part of the tick, and the garbage collections meanwhile.
    /// </summary>
    public sealed class TickProfile
    {
        public enum Phase
        {
            Io,
            World,
            Players,
            Objects,
            Enemies,
            Timers,
            AddRemove,
            Updates,
            Send
        }

        private static readonly string[] Names = { "io", "world", "players", "objects", "enemies", "timers", "add/remove", "updates", "send" };

        private readonly Stopwatch _clock = Stopwatch.StartNew();
        private readonly double[] _phaseMs = new double[Names.Length];
        private double _mark;
        private int _ticks;
        private double _workMs, _workMax, _gapMax, _gapSum;
        private int _slow;
        private int _gen0, _gen1, _gen2;
        private TimeSpan _paused;

        public TickProfile() => Reset();

        private double Now => _clock.Elapsed.TotalMilliseconds;

        /// <summary>Start timing a stretch of work.</summary>
        public void Begin() => _mark = Now;

        /// <summary>The time since the last mark (or Begin) was spent on this phase.</summary>
        public void Mark(Phase phase)
        {
            var now = Now;
            _phaseMs[(int)phase] += now - _mark;
            _mark = now;
        }

        /// <summary>A tick took this long, this long after the one before.</summary>
        public void Tick(double workMs, double gapMs)
        {
            _ticks++;
            _workMs += workMs;
            _workMax = Math.Max(_workMax, workMs);
            _gapSum += gapMs;
            _gapMax = Math.Max(_gapMax, gapMs);
            if (gapMs > 150)
                _slow++;
        }

        /// <summary>The summary since the last one, then start a new period.</summary>
        public string Report()
        {
            var n = Math.Max(_ticks, 1);
            var sb = new StringBuilder();
            sb.Append($"{_ticks} ticks, work avg {_workMs / n:0.00} ms max {_workMax:0} ms, gap avg {_gapSum / n:0} ms max {_gapMax:0} ms, {_slow} over 150 ms; per tick:");
            for (var i = 0; i < Names.Length; i++)
                sb.Append($" {Names[i]} {_phaseMs[i] / n:0.00}");
            sb.Append($"; GCs {GC.CollectionCount(0) - _gen0}/{GC.CollectionCount(1) - _gen1}/{GC.CollectionCount(2) - _gen2} pausing {(GC.GetTotalPauseDuration() - _paused).TotalMilliseconds:0} ms (last full {LastFullPause():0} ms), heap {GC.GetTotalMemory(false) >> 20} MB, process {Environment.WorkingSet >> 20} MB");
            Reset();
            return sb.ToString();
        }

        private void Reset()
        {
            Array.Clear(_phaseMs);
            _ticks = 0;
            _workMs = _workMax = _gapMax = _gapSum = 0;
            _slow = 0;
            _gen0 = GC.CollectionCount(0);
            _gen1 = GC.CollectionCount(1);
            _gen2 = GC.CollectionCount(2);
            _paused = GC.GetTotalPauseDuration();
        }

        private static double LastFullPause()
        {
            var info = GC.GetGCMemoryInfo(GCKind.FullBlocking);
            var total = 0.0;
            foreach (var p in info.PauseDurations)
                total += p.TotalMilliseconds;
            return total;
        }
    }
}
