using System.Runtime.InteropServices;
using WorldServer.core;

namespace WorldServer.utils
{
	// Linux (systemd, Docker): SIGTERM (systemctl stop) and SIGINT stop the server as Ctrl+C does
	// on Windows, so it saves everyone before it exits.
	public sealed class SignalListenerLinux : SignalListener
	{
		private readonly PosixSignalRegistration Term;
		private readonly PosixSignalRegistration Interrupt;

		public SignalListenerLinux(GameServer gameServer) : base(gameServer)
		{
			Term = PosixSignalRegistration.Create(PosixSignal.SIGTERM, OnSignal);
			Interrupt = PosixSignalRegistration.Create(PosixSignal.SIGINT, OnSignal);
		}

		private void OnSignal(PosixSignalContext context)
		{
			context.Cancel = true; // don't exit yet: Run() ends its loop and disposes
			GameServer.Stop();
		}
	}

	public sealed class SignalListenerWindows : SignalListener
	{
		public enum CtrlTypes
        {
            CTRL_C_EVENT = 0,
            CTRL_BREAK_EVENT = 1,
            CTRL_CLOSE_EVENT = 2,
            CTRL_LOGOFF_EVENT = 5,
            CTRL_SHUTDOWN_EVENT = 6
        }

        [DllImport("Kernel32")]
        public static extern bool SetConsoleCtrlHandler(HandlerRoutine Handler, bool Add);

        public delegate bool HandlerRoutine(CtrlTypes CtrlType);

        private readonly HandlerRoutine ConsoleCtrlCheckRoutine;

		public SignalListenerWindows(GameServer gameServer) : base(gameServer) {


            ConsoleCtrlCheckRoutine = ConsoleCtrlCheck;
            SetConsoleCtrlHandler(ConsoleCtrlCheckRoutine, true);
        }

        private bool ConsoleCtrlCheck(CtrlTypes ctrlType)
        {
            switch (ctrlType)
            {
                case CtrlTypes.CTRL_LOGOFF_EVENT:
                case CtrlTypes.CTRL_SHUTDOWN_EVENT:
                case CtrlTypes.CTRL_C_EVENT:
                case CtrlTypes.CTRL_BREAK_EVENT:
                case CtrlTypes.CTRL_CLOSE_EVENT:
                    GameServer.Stop();
                    break;
            }
            return true;
        }
	}

    public abstract class SignalListener
    {
		protected readonly GameServer GameServer;

        public SignalListener(GameServer gameServer) => GameServer = gameServer;
    }
}
