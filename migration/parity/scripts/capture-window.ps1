# Capture the client area of a window by title + PID to a PNG.
# Uses PrintWindow with PW_RENDERFULLCONTENT (2) for DirectX/GPUI surfaces.
param(
  [Parameter(Mandatory = $true)][int]$ProcessId,
  [Parameter(Mandatory = $true)][string]$Title,
  [Parameter(Mandatory = $true)][string]$OutPath
)

Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
using System.Text;

public static class ParityCapture {
  public const uint PW_RENDERFULLCONTENT = 2;
  public const int GWL_STYLE = -16;
  public const int WS_CHILD = 0x40000000;

  public delegate bool EnumProc(IntPtr hWnd, IntPtr lParam);

  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc lpEnumFunc, IntPtr lParam);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint lpdwProcessId);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr hWnd, StringBuilder lpString, int nMaxCount);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hWnd);
  [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr hWnd, out RECT lpRect);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT lpRect);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr hwnd, IntPtr hdcBlt, uint nFlags);
  [DllImport("user32.dll")] public static extern IntPtr GetWindowDC(IntPtr hWnd);
  [DllImport("user32.dll")] public static extern int ReleaseDC(IntPtr hWnd, IntPtr hDC);
  [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr hWnd, ref POINT lpPoint);
  [DllImport("user32.dll")] public static extern int GetWindowLong(IntPtr hWnd, int nIndex);
  [DllImport("gdi32.dll")] public static extern IntPtr CreateCompatibleDC(IntPtr hdc);
  [DllImport("gdi32.dll")] public static extern IntPtr CreateCompatibleBitmap(IntPtr hdc, int nWidth, int nHeight);
  [DllImport("gdi32.dll")] public static extern IntPtr SelectObject(IntPtr hdc, IntPtr hgdiobj);
  [DllImport("gdi32.dll")] public static extern bool DeleteObject(IntPtr hObject);
  [DllImport("gdi32.dll")] public static extern bool DeleteDC(IntPtr hdc);

  [StructLayout(LayoutKind.Sequential)]
  public struct RECT { public int Left, Top, Right, Bottom; }

  [StructLayout(LayoutKind.Sequential)]
  public struct POINT { public int X, Y; }

  public static IntPtr Find(uint pid, string title) {
    IntPtr found = IntPtr.Zero;
    EnumWindows((hWnd, lParam) => {
      if (!IsWindowVisible(hWnd)) return true;
      uint windowPid;
      GetWindowThreadProcessId(hWnd, out windowPid);
      if (windowPid != pid) return true;
      int style = GetWindowLong(hWnd, GWL_STYLE);
      if ((style & WS_CHILD) != 0) return true;
      var sb = new StringBuilder(512);
      GetWindowText(hWnd, sb, sb.Capacity);
      if (sb.ToString() == title) {
        found = hWnd;
        return false;
      }
      return true;
    }, IntPtr.Zero);
    return found;
  }
}
"@

$hwnd = [ParityCapture]::Find([uint32]$ProcessId, $Title)
if ($hwnd -eq [IntPtr]::Zero) {
  Write-Error "No visible top-level window titled '$Title' for PID $ProcessId"
  exit 2
}

$windowRect = New-Object ParityCapture+RECT
[void][ParityCapture]::GetWindowRect($hwnd, [ref]$windowRect)
$winW = $windowRect.Right - $windowRect.Left
$winH = $windowRect.Bottom - $windowRect.Top
if ($winW -le 0 -or $winH -le 0) {
  Write-Error "Window rect is empty ($winW x $winH)"
  exit 3
}

$clientRect = New-Object ParityCapture+RECT
[void][ParityCapture]::GetClientRect($hwnd, [ref]$clientRect)
$clientW = $clientRect.Right - $clientRect.Left
$clientH = $clientRect.Bottom - $clientRect.Top

$clientOrigin = New-Object ParityCapture+POINT
$clientOrigin.X = 0
$clientOrigin.Y = 0
[void][ParityCapture]::ClientToScreen($hwnd, [ref]$clientOrigin)
$cropX = $clientOrigin.X - $windowRect.Left
$cropY = $clientOrigin.Y - $windowRect.Top

$hdcWindow = [ParityCapture]::GetWindowDC($hwnd)
$hdcMem = [ParityCapture]::CreateCompatibleDC($hdcWindow)
$hbm = [ParityCapture]::CreateCompatibleBitmap($hdcWindow, $winW, $winH)
$old = [ParityCapture]::SelectObject($hdcMem, $hbm)

$ok = [ParityCapture]::PrintWindow($hwnd, $hdcMem, [ParityCapture]::PW_RENDERFULLCONTENT)
if (-not $ok) {
  [void][ParityCapture]::SelectObject($hdcMem, $old)
  [void][ParityCapture]::DeleteObject($hbm)
  [void][ParityCapture]::DeleteDC($hdcMem)
  [void][ParityCapture]::ReleaseDC($hwnd, $hdcWindow)
  Write-Error "PrintWindow failed"
  exit 4
}

$full = [System.Drawing.Image]::FromHbitmap($hbm)
$cropped = New-Object System.Drawing.Bitmap $clientW, $clientH
$g = [System.Drawing.Graphics]::FromImage($cropped)
$src = New-Object System.Drawing.Rectangle $cropX, $cropY, $clientW, $clientH
$dst = New-Object System.Drawing.Rectangle 0, 0, $clientW, $clientH
$g.DrawImage($full, $dst, $src, [System.Drawing.GraphicsUnit]::Pixel)
$g.Dispose()
$full.Dispose()

$dir = Split-Path -Parent $OutPath
if ($dir -and -not (Test-Path $dir)) { New-Item -ItemType Directory -Force -Path $dir | Out-Null }
$cropped.Save($OutPath, [System.Drawing.Imaging.ImageFormat]::Png)
$cropped.Dispose()

[void][ParityCapture]::SelectObject($hdcMem, $old)
[void][ParityCapture]::DeleteObject($hbm)
[void][ParityCapture]::DeleteDC($hdcMem)
[void][ParityCapture]::ReleaseDC($hwnd, $hdcWindow)

$check = [System.Drawing.Bitmap]::FromFile($OutPath)
$sum = [int64]0
$step = [Math]::Max(1, [Math]::Floor($check.Width / 40))
$count = 0
for ($y = 0; $y -lt $check.Height; $y += $step) {
  for ($x = 0; $x -lt $check.Width; $x += $step) {
    $c = $check.GetPixel($x, $y)
    $sum += [int]$c.R + [int]$c.G + [int]$c.B
    $count++
  }
}
$check.Dispose()
$mean = if ($count -gt 0) { $sum / ($count * 3.0) } else { 0 }
Write-Output "captured=${clientW}x${clientH} meanLuma=$([Math]::Round($mean,2)) path=$OutPath"
if ($mean -lt 1.0) {
  Write-Error "PNG looks black (mean luma $mean). PrintWindow may have failed for this GPU surface."
  exit 5
}
exit 0
