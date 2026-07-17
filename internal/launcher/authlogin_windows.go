package launcher

import (
	"os/exec"
	"strconv"
	"strings"
)

const uiaLoginScript = `
$ProgressPreference = 'SilentlyContinue'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -AssemblyName System.Windows.Forms
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class Win {
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c);
  [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr h);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint pid);
  [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
  [DllImport("user32.dll")] public static extern bool AttachThreadInput(uint idAttach, uint idAttachTo, bool fAttach);
  [StructLayout(LayoutKind.Sequential)] public struct MOUSEINPUT { public int dx; public int dy; public uint mouseData; public uint dwFlags; public uint time; public IntPtr dwExtraInfo; }
  [StructLayout(LayoutKind.Sequential)] public struct KEYBDINPUT { public ushort wVk; public ushort wScan; public uint dwFlags; public uint time; public IntPtr dwExtraInfo; }
  [StructLayout(LayoutKind.Sequential)] public struct HARDWAREINPUT { public uint uMsg; public ushort wParamL; public ushort wParamH; }
  [StructLayout(LayoutKind.Explicit)] public struct InputUnion { [FieldOffset(0)] public MOUSEINPUT mi; [FieldOffset(0)] public KEYBDINPUT ki; [FieldOffset(0)] public HARDWAREINPUT hi; }
  [StructLayout(LayoutKind.Sequential)] public struct INPUT { public uint type; public InputUnion u; }
  [DllImport("user32.dll", SetLastError=true)] public static extern uint SendInput(uint nInputs, INPUT[] pInputs, int cbSize);
  public static void TypeUnicode(string s) {
    foreach (char c in s) {
      INPUT[] inp = new INPUT[2];
      inp[0].type = 1; inp[0].u.ki.wScan = c; inp[0].u.ki.dwFlags = 0x0004;
      inp[1].type = 1; inp[1].u.ki.wScan = c; inp[1].u.ki.dwFlags = 0x0006;
      SendInput(2, inp, Marshal.SizeOf(typeof(INPUT)));
      System.Threading.Thread.Sleep(6);
    }
  }
}
"@
$login = $env:AC_IMPORT_LOGIN
$password = $env:AC_IMPORT_PASS
$deadline = [DateTime]::UtcNow.AddSeconds({{TIMEOUT}})
$loginEl = $null; $passEl = $null; $btn = $null; $hwnd = [IntPtr]::Zero
while ([DateTime]::UtcNow -lt $deadline -and ($null -eq $btn)) {
  Start-Sleep -Milliseconds 400
  foreach ($p in Get-Process -ErrorAction SilentlyContinue) {
    if ($p.MainWindowHandle -eq [IntPtr]::Zero) { continue }
    try {
      $r = [Windows.Automation.AutomationElement]::FromHandle($p.MainWindowHandle)
      if ($r.Current.Name -ne 'Cristalix') { continue }
      $bcond = [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty, 'Вход')
      $b = $r.FindFirst([Windows.Automation.TreeScope]::Descendants, $bcond)
      if ($null -eq $b -or $b.Current.ControlType -ne [Windows.Automation.ControlType]::Button) { continue }
      $editCond = [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ControlTypeProperty, [Windows.Automation.ControlType]::Edit)
      $edits = $r.FindAll([Windows.Automation.TreeScope]::Descendants, $editCond)
      $list = @()
      for ($i = 0; $i -lt $edits.Count; $i++) {
        $e = $edits.Item($i)
        if (-not $e.Current.IsEnabled) { continue }
        $rc = $e.Current.BoundingRectangle
        if ($rc.Width -le 1) { continue }
        $isPass = $false
        try { $isPass = [bool]$e.GetCurrentPropertyValue([Windows.Automation.AutomationElement]::IsPasswordProperty) } catch {}
        $list += [pscustomobject]@{ El = $e; Y = [int]$rc.Y; Pass = $isPass; Name = $e.Current.Name }
      }
      $list = @($list | Sort-Object Y)
      foreach ($x in $list) { Write-Output ("login: edit y=" + $x.Y + " pass=" + $x.Pass + " name='" + $x.Name + "'") }
      $pItem = $list | Where-Object { $_.Pass } | Select-Object -First 1
      $lItem = $list | Where-Object { -not $_.Pass } | Select-Object -First 1
      if ($lItem) { $loginEl = $lItem.El }
      if ($pItem) { $passEl = $pItem.El } elseif ($list.Count -ge 2) { $passEl = $list[1].El }
      if ($loginEl -and $passEl) { $btn = $b; $hwnd = $p.MainWindowHandle; break }
    } catch {}
  }
}
if ($null -eq $btn) { Write-Output "login: form NOT found (no Cristalix window with Вход button + 2 edits)"; exit 1 }
function Get-Value($el) {
  try { return $el.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).Current.Value } catch { return '<n/a>' }
}
function Convert-Keys($s) {
  $out = ''
  foreach ($ch in $s.ToCharArray()) {
    if ('+^%~(){}[]'.IndexOf($ch) -ge 0) { $out += '{' + $ch + '}' } else { $out += $ch }
  }
  return $out
}
function Ensure-Focus($el) {
  for ($i = 0; $i -lt 12; $i++) {
    try { $el.SetFocus() } catch {}
    Start-Sleep -Milliseconds 150
    try { if ($el.Current.HasKeyboardFocus) { return $true } } catch {}
  }
  return $false
}
function Launcher-Owns-Foreground($h) {
  $fg = [Win]::GetForegroundWindow()
  if ($fg -eq [IntPtr]::Zero) { return $false }
  if ($fg -eq $h) { return $true }
  # A launcher-owned popup (e.g. the login autocomplete dropdown) still belongs to the
  # launcher - only refuse to type when the foreground window is a DIFFERENT process.
  $lp = [uint32]0; [void][Win]::GetWindowThreadProcessId($h, [ref]$lp)
  $fp = [uint32]0; [void][Win]::GetWindowThreadProcessId($fg, [ref]$fp)
  return ($lp -ne 0 -and $lp -eq $fp)
}
function Foreground-Launcher($h) {
  for ($i = 0; $i -lt 15; $i++) {
    if (Launcher-Owns-Foreground $h) { return $true }
    # Defeat Windows' foreground-lock: attach our input to the current foreground
    # thread so SetForegroundWindow is allowed to actually take effect.
    $fore = [Win]::GetForegroundWindow()
    $ft = [uint32]0; $foreThread = [Win]::GetWindowThreadProcessId($fore, [ref]$ft)
    $appThread = [Win]::GetCurrentThreadId()
    $attached = $false
    if ($foreThread -ne 0 -and $foreThread -ne $appThread) {
      $attached = [Win]::AttachThreadInput($foreThread, $appThread, $true)
    }
    [Win]::ShowWindow($h, 9) | Out-Null
    [Win]::BringWindowToTop($h) | Out-Null
    [Win]::SetForegroundWindow($h) | Out-Null
    if ($attached) { [Win]::AttachThreadInput($foreThread, $appThread, $false) | Out-Null }
    Start-Sleep -Milliseconds 140
    if (Launcher-Owns-Foreground $h) { return $true }
  }
  return (Launcher-Owns-Foreground $h)
}
function Set-Field($el, $text, $label, $h) {
  # SAFETY: fill ONLY when (1) the launcher process owns the foreground window and
  # (2) the target field actually holds keyboard focus. Never send input to another app.
  if (-not (Foreground-Launcher $h)) { Write-Output ($label + ": ABORT (launcher not foreground)"); exit 2 }
  $f = Ensure-Focus $el
  if (-not $f) { Write-Output ($label + ": ABORT (field has no keyboard focus)"); exit 2 }
  if (-not (Launcher-Owns-Foreground $h)) { Write-Output ($label + ": ABORT (foreground left launcher)"); exit 2 }
  try {
    # Clear (Ctrl+A/Del are VK-based, layout-independent) then type via unicode key events.
    [System.Windows.Forms.SendKeys]::SendWait("^a")
    Start-Sleep -Milliseconds 40
    [System.Windows.Forms.SendKeys]::SendWait("{DEL}")
    Start-Sleep -Milliseconds 40
    if (-not (Launcher-Owns-Foreground $h)) { Write-Output ($label + ": ABORT (foreground left before type)"); exit 2 }
    [Win]::TypeUnicode($text)
    Start-Sleep -Milliseconds 60
    Write-Output ($label + ": typed (kbFocus=" + $f + ")")
  } catch { Write-Output ($label + ": FAILED " + $_); exit 3 }
}
if (-not (Foreground-Launcher $hwnd)) { Write-Output "login: ABORT (cannot bring launcher to foreground)"; exit 2 }
Start-Sleep -Milliseconds 300
Set-Field $loginEl $login 'login-field' $hwnd
Start-Sleep -Milliseconds 300
Write-Output ("login-field value now='" + (Get-Value $loginEl) + "'")
Set-Field $passEl $password 'pass-field' $hwnd
Start-Sleep -Milliseconds 200
try {
  $inv = $btn.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern)
  $inv.Invoke()
  Write-Output "login: invoked Вход"
} catch { Write-Output "login: Invoke failed" }
Start-Sleep -Milliseconds 3000
try {
  $r2 = [Windows.Automation.AutomationElement]::FromHandle($hwnd)
  $txtCond = [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ControlTypeProperty, [Windows.Automation.ControlType]::Text)
  $txts = $r2.FindAll([Windows.Automation.TreeScope]::Descendants, $txtCond)
  $msgs = @()
  for ($i = 0; $i -lt $txts.Count; $i++) { $n = $txts.Item($i).Current.Name; if ($n) { $msgs += $n } }
  Write-Output ("after-invoke texts: [" + ($msgs -join ' | ') + "]")
  $joined = ($msgs -join ' ')
  if ($joined -match '(еправильн|еверн)' -and $joined -match '(логин|парол)') {
    Write-Output "login: wrong credentials detected"
    exit 4
  }
  $still = $r2.FindFirst([Windows.Automation.TreeScope]::Descendants, [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty, 'Вход'))
  Write-Output ("after-invoke login-button-still-present=" + ($null -ne $still))
} catch {}
exit 0
`

func UiaLogin(login, password string, timeoutSec int) (int, string) {
	script := strings.Replace(uiaLoginScript, "{{TIMEOUT}}", strconv.Itoa(timeoutSec), 1)
	cmd := exec.Command("powershell", "-NoProfile", "-NonInteractive", "-EncodedCommand", encodePowershell(script))
	cmd.Env = append(CleanEnv(), "AC_IMPORT_LOGIN="+login, "AC_IMPORT_PASS="+password)
	detach(cmd)
	out, err := cmd.CombinedOutput()
	msg := strings.TrimSpace(string(out))
	if err != nil {
		if ee, ok := err.(*exec.ExitError); ok {
			return ee.ExitCode(), msg
		}
		return -1, msg + " | run error: " + err.Error()
	}
	return 0, msg
}
