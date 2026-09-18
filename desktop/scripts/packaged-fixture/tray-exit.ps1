param([int]$TimeoutSec = 30, [int]$TargetPid = 0)
$ErrorActionPreference = "Stop"
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class TrayMenu {
  [DllImport("user32.dll", CharSet=CharSet.Unicode, SetLastError=true)]
  public static extern IntPtr FindWindowW(string cls, string name);
  [DllImport("user32.dll")]
  public static extern bool PostMessageW(IntPtr h, uint m, IntPtr w, IntPtr l);
  [DllImport("user32.dll")]
  public static extern IntPtr SendMessageW(IntPtr h, uint m, IntPtr w, IntPtr l);
  [DllImport("user32.dll")]
  public static extern int GetMenuItemCount(IntPtr hmenu);
  [DllImport("user32.dll")]
  public static extern bool GetMenuItemRect(IntPtr hwnd, IntPtr hmenu, uint item, out RECT rect);
  [DllImport("user32.dll")]
  public static extern bool GetWindowRect(IntPtr h, out RECT rect);
  [DllImport("user32.dll")]
  public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")]
  public static extern bool SetProcessDPIAware();
  [StructLayout(LayoutKind.Sequential)]
  public struct RECT { public int Left, Top, Right, Bottom; }
  public static string LastRect = "";
  [DllImport("user32.dll")]
  public static extern bool EnumWindows(EnumProc cb, IntPtr l);
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")]
  public static extern IntPtr GetWindow(IntPtr h, uint rel);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)]
  public static extern int GetClassNameW(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")]
  public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")]
  public static extern bool IsWindow(IntPtr h);
  [DllImport("user32.dll")]
  public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  // TrackPopupMenu's #32768 popup has no GW_OWNER — match by the owning
  // process of the tray window instead.
  public static IntPtr FindMenuForWindow(IntPtr owner) {
    uint ownerPid; GetWindowThreadProcessId(owner, out ownerPid);
    IntPtr found = IntPtr.Zero;
    EnumWindows(delegate(IntPtr h, IntPtr l) {
      StringBuilder sb = new StringBuilder(64);
      GetClassNameW(h, sb, 64);
      if (sb.ToString() == "#32768" && IsWindowVisible(h)) {
        uint pid; GetWindowThreadProcessId(h, out pid);
        if (pid == ownerPid) { found = h; return false; }
      }
      return true;
    }, IntPtr.Zero);
    return found;
  }
  // Other worktrees may run kepler-backend with a tray of the same class —
  // only the window owned by $TargetPid is ours to click.
  public static IntPtr FindTrayForPid(uint targetPid) {
    IntPtr found = IntPtr.Zero;
    EnumWindows(delegate(IntPtr h, IntPtr l) {
      StringBuilder sb = new StringBuilder(64);
      GetClassNameW(h, sb, 64);
      if (sb.ToString() == "KosmosBackendTray") {
        uint pid; GetWindowThreadProcessId(h, out pid);
        if (pid == targetPid) { found = h; return false; }
      }
      return true;
    }, IntPtr.Zero);
    return found;
  }
  // Posted keys/clicks are flaky: the menu loop hit-tests MSG.pt, not lParam.
  // The #32768 popup instead accepts internal menu messages by item index:
  // MN_BUTTONDOWN (0x01ED) then MN_BUTTONUP (0x01EF) execute the item the way
  // the modal loop does after a real click. Runtime appends "Exit" last.
  public static bool ClickLastItem(IntPtr popup) {
    IntPtr hmenu = SendMessageW(popup, 0x01E1, IntPtr.Zero, IntPtr.Zero); // MN_GETHMENU
    if (hmenu == IntPtr.Zero) return false;
    int count = GetMenuItemCount(hmenu);
    if (count <= 0) return false;
    LastRect = "items=" + count;
    IntPtr last = (IntPtr)(count - 1);
    SendMessageW(popup, 0x01ED, last, IntPtr.Zero); // MN_BUTTONDOWN
    SendMessageW(popup, 0x01EF, last, IntPtr.Zero); // MN_BUTTONUP
    return true;
  }
}
"@
# Align our coordinate space with the DPI-aware backend so item rects and
# SetCursorPos agree.
[TrayMenu]::SetProcessDPIAware() | Out-Null
$tray = [IntPtr]::Zero
$deadline = (Get-Date).AddSeconds($TimeoutSec)
while ((Get-Date) -lt $deadline) {
  $tray = [TrayMenu]::FindTrayForPid([uint32]$TargetPid)
  if ($tray -ne [IntPtr]::Zero) { break }
  Start-Sleep -Milliseconds 200
}
if ($tray -eq [IntPtr]::Zero) { Write-Error "KosmosBackendTray window not found for pid $TargetPid"; exit 2 }
# A dismissed popup can close without executing the item — success is the tray
# window itself being destroyed (backend exiting). Reopen + click until then.
$done = $false
for ($cycle = 0; $cycle -lt 5 -and -not $done; $cycle++) {
  # WM_TRAY_CALLBACK = WM_USER + 1 = 0x0401; icon uID = 1; WM_RBUTTONUP = 0x0205
  [TrayMenu]::PostMessageW($tray, 0x0401, [IntPtr]1, [IntPtr]0x0205) | Out-Null
  $popup = [IntPtr]::Zero
  $deadline = (Get-Date).AddSeconds(10)
  while ((Get-Date) -lt $deadline) {
    $popup = [TrayMenu]::FindMenuForWindow($tray)
    if ($popup -ne [IntPtr]::Zero) { break }
    if (-not [TrayMenu]::IsWindow($tray)) { $done = $true; break }
    Start-Sleep -Milliseconds 100
  }
  if ($done) { break }
  if ($popup -eq [IntPtr]::Zero) { Write-Error "tray context menu did not open"; exit 3 }
  $clicked = [TrayMenu]::ClickLastItem($popup)
  Write-Output "cycle $cycle popup=$popup clicked=$clicked $([TrayMenu]::LastRect)"
  Start-Sleep -Milliseconds 500
  $done = -not [TrayMenu]::IsWindow($tray)
}
if (-not $done) { Write-Error "tray menu item was not selected"; exit 4 }
exit 0
