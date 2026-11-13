# Where a window would move from, without touching the mouse.
#
# `gpui` answers `WM_NCHITTEST` (the message that decides `HTCAPTION`, so a
# native window move, and `HTMINBUTTON`/`HTMAXBUTTON`/`HTCLOSE`) from the
# hitboxes under the pointer, and it learns what is under the pointer from the
# last `WM_MOUSEMOVE` it saw. Sending that move to the window puts the hit test
# at a chosen point while the real cursor stays where it is.
#
#   pwsh manager/scripts/window-hit-test.ps1 -Exe manager\target\debug\manager.exe
#
# Prints one line per probe: the control code the window would answer there.
param(
  # The executable to start and probe, or the process of a window already open
  # (`-Id`: `nsh run`, a release build). One of the two.
  [string]$Exe = '',
  [int]$Id = 0,
  [string[]]$Point = @()
)

$ErrorActionPreference = 'Stop'
Add-Type @'
using System;
using System.Runtime.InteropServices;
public class HitTest {
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool GetCursorPos(out POINT p);
  [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr h, uint msg, IntPtr w, IntPtr l);
  public struct RECT { public int L, T, R, B; }
  public struct POINT { public int X, Y; }
}
'@

$WM_MOUSEMOVE = 0x0200
$WM_NCHITTEST = 0x0084
$WM_MOUSELEAVE = 0x02A3

function LParam([int]$x, [int]$y) {
  return [IntPtr]((($y -band 0xFFFF) -shl 16) -bor ($x -band 0xFFFF))
}

function Code([long]$value) {
  switch ($value) {
    1 { 'HTCLIENT' }
    2 { 'HTCAPTION' }
    8 { 'HTMINBUTTON' }
    9 { 'HTMAXBUTTON' }
    20 { 'HTCLOSE' }
    default { "code $value" }
  }
}

if ($Exe -eq '' -and $Id -eq 0) { throw 'pass -Exe <path> or -Id <process id>' }

$started = $Id -eq 0
$process = if ($started) { Start-Process -FilePath $Exe -PassThru } else { Get-Process -Id $Id }
try {
  for ($i = 0; $i -lt 40 -and $process.MainWindowHandle -eq 0; $i++) { Start-Sleep -Milliseconds 250 }
  $handle = $process.MainWindowHandle
  if ($handle -eq 0) { throw "no window from $($process.ProcessName)" }
  Start-Sleep -Milliseconds 1500

  $rect = New-Object HitTest+RECT
  [HitTest]::GetWindowRect($handle, [ref]$rect) | Out-Null
  $width = $rect.R - $rect.L
  $height = $rect.B - $rect.T
  "window $width x $height at $($rect.L),$($rect.T)"

  $probes = if ($Point.Count -gt 0) { $Point } else {
    @(
      'heading=700,30',
      'sidebar=150,700',
      'page=700,300',
      'min=W-115,18',
      'max=W-69,18',
      'close=W-23,18'
    )
  }

  $real = New-Object HitTest+POINT
  [HitTest]::GetCursorPos([ref]$real) | Out-Null

  $rows = foreach ($probe in $probes) {
    $name, $coords = $probe -split '=', 2
    $px, $py = $coords -split ',', 2
    $x = if ($px -match '^W([+-]\d+)$') { $width + [int]$Matches[1] } else { [int]$px }
    $y = [int]$py
    # The move is in client pixels; the hit test is in screen ones.
    [HitTest]::SendMessage($handle, $WM_MOUSEMOVE, [IntPtr]::Zero, (LParam $x $y)) | Out-Null
    $answer = [HitTest]::SendMessage($handle, $WM_NCHITTEST, [IntPtr]::Zero, (LParam ($rect.L + $x) ($rect.T + $y))).ToInt64()
    [pscustomobject]@{ probe = $name; at = "$x,$y"; answers = (Code $answer) }
  }
  $rows | Format-Table -AutoSize | Out-String -Width 120

  # Put the hit test back under the real cursor so the window's hover state is
  # not left believing the probe point.
  $inside = $real.X -ge $rect.L -and $real.X -lt $rect.R -and $real.Y -ge $rect.T -and $real.Y -lt $rect.B
  if ($inside) {
    $back = LParam ($real.X - $rect.L) ($real.Y - $rect.T)
    [HitTest]::SendMessage($handle, $WM_MOUSEMOVE, [IntPtr]::Zero, $back) | Out-Null
  } else {
    [HitTest]::SendMessage($handle, $WM_MOUSELEAVE, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null
  }
}
finally {
  if ($started -and -not $process.HasExited) { Stop-Process -Id $process.Id -Force }
}
