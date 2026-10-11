# Drives the test client window ("betterSkillys port test", built by build_test_client.py).
# Never use it on the real client ("betterSkillys"): that window may be someone's game.
#
#   testclient.ps1 start              start the test client (closes a running one first)
#   testclient.ps1 enter              click Play, then the first character (default window size)
#   testclient.ps1 send '#fps 1000' 'hello'   lines for TestHook (autochat.txt)
#   testclient.ps1 front | max | restore      bring to front / maximise / restore the window
#   testclient.ps1 shot <file.png>    screenshot of the window
#   testclient.ps1 click <x>,<y>      click at client coordinates
#   testclient.ps1 idle               milliseconds since the last keyboard/mouse input on this PC
#   testclient.ps1 close
#
# The AIR runtime stops running frames while its window is covered or the display is asleep,
# so the window must be in front for anything to happen (and for screenshots). Bringing it to
# the front takes over the screen: check "idle" first, and ask before doing it while someone
# is using the PC. Clicks work through window messages; keystrokes don't (AIR ignores them),
# which is what the autochat hook is for.
param(
    [Parameter(Mandatory = $true, Position = 0)]
    [ValidateSet('start', 'enter', 'send', 'front', 'max', 'restore', 'shot', 'click', 'idle', 'close')]
    [string]$Action,
    [Parameter(Position = 1, ValueFromRemainingArguments = $true)]
    [string[]]$Rest = @()
)

$ErrorActionPreference = 'Stop'
$Title = 'betterSkillys port test'
$Root = Resolve-Path (Join-Path $PSScriptRoot '..\..')
$Store = Join-Path $env:APPDATA 'betterSkillysPortTest\Local Store'

Add-Type @"
using System; using System.Runtime.InteropServices; using System.Collections.Generic;
public class TestWin {
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, IntPtr p);
  [DllImport("user32.dll")] public static extern bool AttachThreadInput(uint a, uint b, bool f);
  [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumChildWindows(IntPtr h, EnumProc f, IntPtr l);
  [StructLayout(LayoutKind.Sequential)] struct LII { public uint cbSize; public uint dwTime; }
  [DllImport("user32.dll")] static extern bool GetLastInputInfo(ref LII p);
  public struct RECT { public int L, T, R, B; }
  public static uint IdleMs() { LII l = new LII(); l.cbSize = 8; GetLastInputInfo(ref l); return (uint)Environment.TickCount - l.dwTime; }
  public static IntPtr LastChild(IntPtr h) { IntPtr r = h; EnumChildWindows(h, (c, x) => { r = c; return true; }, IntPtr.Zero); return r; }
}
"@

function Get-Window {
    $p = Get-Process adl -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowTitle -eq $Title } | Select-Object -First 1
    if (-not $p) { throw "test client window not found" }
    return $p.MainWindowHandle
}

function Send-Click([IntPtr]$h, [int]$x, [int]$y) {
    $target = [TestWin]::LastChild($h)
    $l = [IntPtr](($y -shl 16) -bor $x)
    [TestWin]::PostMessage($target, 0x200, [IntPtr]0, $l) | Out-Null; Start-Sleep -Milliseconds 80
    [TestWin]::PostMessage($target, 0x201, [IntPtr]1, $l) | Out-Null; Start-Sleep -Milliseconds 80
    [TestWin]::PostMessage($target, 0x202, [IntPtr]0, $l) | Out-Null
}

switch ($Action) {
    'start' {
        Get-Process adl -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowTitle -eq $Title } | Stop-Process
        Start-Sleep -Seconds 1
        Remove-Item (Join-Path $Store 'errors.txt') -ErrorAction SilentlyContinue
        $test = Join-Path $Root 'build\test'
        Start-Process -FilePath (Join-Path $Root 'tools\airsdk\bin\adl.exe') -ArgumentList 'test-app.xml', '.' -WorkingDirectory $test
        $deadline = (Get-Date).AddSeconds(60)
        while ((Get-Date) -lt $deadline) {
            if (Get-Process adl -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowTitle -eq $Title }) { 'started'; return }
            Start-Sleep -Milliseconds 500
        }
        throw 'test client did not start'
    }
    'enter' {
        $h = Get-Window
        Send-Click $h 408 550   # Play
        Start-Sleep -Seconds 5
        Send-Click $h 292 137   # first character
        'entering'
    }
    'send' {
        New-Item -ItemType Directory -Force $Store | Out-Null
        Set-Content -Path (Join-Path $Store 'autochat.txt') -Value $Rest -Encoding ascii
        'sent'
    }
    'front' {
        $h = Get-Window
        $fg = [TestWin]::GetForegroundWindow()
        $t1 = [TestWin]::GetWindowThreadProcessId($fg, [IntPtr]::Zero); $t2 = [TestWin]::GetCurrentThreadId()
        [TestWin]::AttachThreadInput($t2, $t1, $true) | Out-Null
        [TestWin]::BringWindowToTop($h) | Out-Null
        [TestWin]::SetForegroundWindow($h) | Out-Null
        [TestWin]::AttachThreadInput($t2, $t1, $false) | Out-Null
        "front: $([TestWin]::GetForegroundWindow() -eq $h)"
    }
    'max' { [TestWin]::ShowWindow((Get-Window), 3) | Out-Null; 'maximised' }
    'restore' { [TestWin]::ShowWindow((Get-Window), 9) | Out-Null; 'restored' }
    'shot' {
        # a screen copy: PrintWindow misses the Stage3D (GPU) view
        Add-Type -AssemblyName System.Drawing
        [TestWin]::SetProcessDPIAware() | Out-Null
        $r = New-Object TestWin+RECT; [TestWin]::GetWindowRect((Get-Window), [ref]$r) | Out-Null
        $bmp = New-Object System.Drawing.Bitmap ($r.R - $r.L), ($r.B - $r.T)
        $g = [System.Drawing.Graphics]::FromImage($bmp)
        $g.CopyFromScreen($r.L, $r.T, 0, 0, $bmp.Size)
        $bmp.Save($Rest[0]); $g.Dispose(); $bmp.Dispose()
        "saved $($Rest[0])"
    }
    'click' { $xy = $Rest[0].Split(','); Send-Click (Get-Window) ([int]$xy[0]) ([int]$xy[1]); 'clicked' }
    'idle' { [TestWin]::IdleMs() }
    'close' { Get-Process adl -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowTitle -eq $Title } | Stop-Process; 'closed' }
}
