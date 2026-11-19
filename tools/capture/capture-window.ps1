# Capture a top-level window (with a margin, so DWM corners and shadow are
# visible) to a PNG, for comparing Silvestris against reference/measurements.
#
#   pwsh tools/capture/capture-window.ps1 -Title "Silvestris Gallery" -Out shot.png [-Margin 16]
#
# Uses DWMWA_EXTENDED_FRAME_BOUNDS (the visible frame, without the invisible
# resize border) and prints the frame rectangle and DPI scale.
param(
    [Parameter(Mandatory)] [string] $Title,
    [Parameter(Mandatory)] [string] $Out,
    [int] $Margin = 16
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class SilvestrisCapture {
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    public delegate bool EnumProc(IntPtr hwnd, IntPtr lparam);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr lparam);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr hwnd, StringBuilder text, int max);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr value);
    [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr hwnd, int attr, out RECT rect, int size);
    public static IntPtr Find(string prefix) {
        IntPtr found = IntPtr.Zero;
        EnumWindows((h, l) => {
            if (!IsWindowVisible(h)) return true;
            var sb = new StringBuilder(512);
            GetWindowTextW(h, sb, 512);
            if (sb.ToString().StartsWith(prefix)) { found = h; return false; }
            return true;
        }, IntPtr.Zero);
        return found;
    }
}
'@
# Per-monitor-v2 awareness so coordinates are physical pixels.
[void][SilvestrisCapture]::SetProcessDpiAwarenessContext([IntPtr]::new(-4))
$hwnd = [SilvestrisCapture]::Find($Title)
if ($hwnd -eq [IntPtr]::Zero) { throw "no visible window titled '$Title*'" }
[void][SilvestrisCapture]::SetForegroundWindow($hwnd)
Start-Sleep -Milliseconds 400
$r = New-Object SilvestrisCapture+RECT
$hr = [SilvestrisCapture]::DwmGetWindowAttribute($hwnd, 9, [ref]$r, 16)  # DWMWA_EXTENDED_FRAME_BOUNDS
if ($hr -ne 0) { throw "DwmGetWindowAttribute failed: $hr" }
$x = $r.Left - $Margin; $y = $r.Top - $Margin
$w = ($r.Right - $r.Left) + 2 * $Margin; $h = ($r.Bottom - $r.Top) + 2 * $Margin
$bmp = New-Object System.Drawing.Bitmap $w, $h
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($x, $y, 0, 0, $bmp.Size)
$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose()
$dpi = [SilvestrisCapture]::GetDpiForWindow($hwnd)
"frame {0},{1} {2}x{3} scale {4} margin {5} -> {6}" -f $r.Left, $r.Top, ($r.Right - $r.Left), ($r.Bottom - $r.Top), ($dpi / 96), $Margin, $Out
