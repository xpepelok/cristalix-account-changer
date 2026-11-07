use std::process::Stdio;

use crate::autoplay::powershell;

const LOGIN_SCRIPT: &str = r#"
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
$loginLabels = @('Вход','Sign in','Sign In','Login','Log in','Log In')
$loginEl = $null; $passEl = $null; $btn = $null; $hwnd = [IntPtr]::Zero
while ([DateTime]::UtcNow -lt $deadline -and ($null -eq $btn)) {
  Start-Sleep -Milliseconds 200
  foreach ($p in Get-Process -ErrorAction SilentlyContinue) {
    if ($p.MainWindowHandle -eq [IntPtr]::Zero) { continue }
    try {
      $r = [Windows.Automation.AutomationElement]::FromHandle($p.MainWindowHandle)
      if ($r.Current.Name -ne 'Cristalix') { continue }
      $btnCond = [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ControlTypeProperty, [Windows.Automation.ControlType]::Button)
      $allBtns = $r.FindAll([Windows.Automation.TreeScope]::Descendants, $btnCond)
      $b = $null
      for ($bi = 0; $bi -lt $allBtns.Count; $bi++) {
        $cand = $allBtns.Item($bi)
        $cn = $cand.Current.Name
        foreach ($ll in $loginLabels) { if ($cn -eq $ll) { $b = $cand; break } }
        if ($null -ne $b) { break }
      }
      if ($null -eq $b) { continue }
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
$winpid = [uint32]0
[void][Win]::GetWindowThreadProcessId($hwnd, [ref]$winpid)
Write-Output ("login: winpid=" + $winpid)
function Get-Value($el) {
  try { return $el.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).Current.Value } catch { return '<n/a>' }
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
  $lp = [uint32]0; [void][Win]::GetWindowThreadProcessId($h, [ref]$lp)
  $fp = [uint32]0; [void][Win]::GetWindowThreadProcessId($fg, [ref]$fp)
  return ($lp -ne 0 -and $lp -eq $fp)
}
function Foreground-Launcher($h) {
  for ($i = 0; $i -lt 15; $i++) {
    if (Launcher-Owns-Foreground $h) { return $true }
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
  if (-not (Foreground-Launcher $h)) { Write-Output ($label + ": ABORT (launcher not foreground)"); exit 2 }
  $f = Ensure-Focus $el
  if (-not $f) { Write-Output ($label + ": ABORT (field has no keyboard focus)"); exit 2 }
  if (-not (Launcher-Owns-Foreground $h)) { Write-Output ($label + ": ABORT (foreground left launcher)"); exit 2 }
  try {
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
$typed = $false
for ($t = 0; $t -lt 3 -and -not $typed; $t++) {
  Set-Field $loginEl $login 'login-field' $hwnd
  Start-Sleep -Milliseconds 200
  if ((Get-Value $loginEl) -eq $login) { $typed = $true }
}
Write-Output ("login-field value now='" + (Get-Value $loginEl) + "' verified=" + $typed)
Set-Field $passEl $password 'pass-field' $hwnd
Start-Sleep -Milliseconds 200
try {
  $inv = $btn.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern)
  $inv.Invoke()
  Write-Output "login: invoked Вход"
} catch { Write-Output "login: Invoke failed" }
$txtCond = [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ControlTypeProperty, [Windows.Automation.ControlType]::Text)
$errDeadline = [DateTime]::UtcNow.AddSeconds(8)
while ([DateTime]::UtcNow -lt $errDeadline) {
  Start-Sleep -Milliseconds 300
  try {
    $r2 = [Windows.Automation.AutomationElement]::FromHandle($hwnd)
    $txts = $r2.FindAll([Windows.Automation.TreeScope]::Descendants, $txtCond)
    $msgs = @()
    for ($i = 0; $i -lt $txts.Count; $i++) { $n = $txts.Item($i).Current.Name; if ($n) { $msgs += $n } }
    $joined = ($msgs -join ' ')
    if ($joined -match 'auth-login') { Write-Output ("login: auth-login error [" + $joined + "]"); exit 5 }
    if ($joined -match '(еправильн|еверн|nvalid|ncorrect|rong)' -and $joined -match '(логин|парол|ogin|assword)') {
      Write-Output "login: wrong credentials detected"; exit 4
    }
    $bb = $r2.FindAll([Windows.Automation.TreeScope]::Descendants, $btnCond)
    $stillLogin = $false
    for ($i = 0; $i -lt $bb.Count; $i++) { if ($loginLabels -contains $bb.Item($i).Current.Name) { $stillLogin = $true; break } }
    if (-not $stillLogin) { Write-Output "login: form gone (logged in)"; exit 0 }
  } catch {}
}
Write-Output "login: no verdict within window"
exit 0
"#;

const LOGOUT_SCRIPT: &str = r#"
$ProgressPreference = 'SilentlyContinue'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
$settingsLabels = @('НАСТРОЙКИ','SETTINGS','Settings')
$logoutLabels = @('Выйти из аккаунта','Log out','Logout','Log Out','Sign out','Sign Out')
$loginLabels = @('Вход','Sign in','Sign In','Login','Log in','Log In')
$deadline = [DateTime]::UtcNow.AddSeconds({{TIMEOUT}})
function Get-Root() {
  foreach ($p in Get-Process -ErrorAction SilentlyContinue) {
    if ($p.MainWindowHandle -eq [IntPtr]::Zero) { continue }
    try {
      $r = [Windows.Automation.AutomationElement]::FromHandle($p.MainWindowHandle)
      if ($r.Current.Name -eq 'Cristalix') { return $r }
    } catch {}
  }
  return $null
}
function Find-Button($root, $labels) {
  if ($null -eq $root) { return $null }
  $cond = [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ControlTypeProperty, [Windows.Automation.ControlType]::Button)
  $btns = $root.FindAll([Windows.Automation.TreeScope]::Descendants, $cond)
  for ($i = 0; $i -lt $btns.Count; $i++) {
    $bn = $btns.Item($i)
    try { if (($labels -contains $bn.Current.Name) -and $bn.Current.IsEnabled) { return $bn } } catch {}
  }
  return $null
}
function Invoke-El($el) {
  try { $el.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke(); return $true } catch { return $false }
}
if (Find-Button (Get-Root) $loginLabels) { Write-Output 'logout: already on login form'; exit 0 }
$clicked = $false
while ([DateTime]::UtcNow -lt $deadline -and -not $clicked) {
  $root = Get-Root
  $l = Find-Button $root $logoutLabels
  if ($l) { if (Invoke-El $l) { $clicked = $true; break } }
  else {
    $s = Find-Button $root $settingsLabels
    if ($s) { Invoke-El $s | Out-Null }
  }
  Start-Sleep -Milliseconds 250
}
if (-not $clicked) { Write-Output 'logout: could not reach logout button'; exit 1 }
while ([DateTime]::UtcNow -lt $deadline) {
  if (Find-Button (Get-Root) $loginLabels) { Write-Output 'logout: back to login form'; exit 0 }
  Start-Sleep -Milliseconds 200
}
Write-Output 'logout: login form did not reappear'; exit 3
"#;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScriptOutcome {
    pub code: i32,
    pub output: String,
}

impl ScriptOutcome {
    pub fn window_pid(&self) -> u32 {
        parse_winpid(&self.output)
    }
}

async fn run(mut command: tokio::process::Command) -> ScriptOutcome {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    match command.output().await {
        Ok(out) => {
            let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
            text.push_str(&String::from_utf8_lossy(&out.stderr));
            ScriptOutcome {
                code: out.status.code().unwrap_or(-1),
                output: text.trim().to_owned(),
            }
        }
        Err(err) => ScriptOutcome {
            code: -1,
            output: format!(" | run error: {err}"),
        },
    }
}

pub async fn uia_login(login: &str, password: &str, timeout: u32) -> ScriptOutcome {
    let script = LOGIN_SCRIPT.replacen("{{TIMEOUT}}", &timeout.to_string(), 1);
    let mut command = powershell(&script);
    command
        .env("AC_IMPORT_LOGIN", login)
        .env("AC_IMPORT_PASS", password);
    run(command).await
}

pub async fn uia_logout(timeout: u32) -> ScriptOutcome {
    let script = LOGOUT_SCRIPT.replacen("{{TIMEOUT}}", &timeout.to_string(), 1);
    run(powershell(&script)).await
}

fn parse_winpid(out: &str) -> u32 {
    const MARKER: &str = "winpid=";
    let Some(index) = out.find(MARKER) else {
        return 0;
    };
    out[index + MARKER.len()..]
        .bytes()
        .take_while(u8::is_ascii_digit)
        .fold(0u32, |acc, digit| {
            acc.saturating_mul(10)
                .saturating_add(u32::from(digit - b'0'))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_pid_is_read_from_script_output() {
        assert_eq!(
            parse_winpid("login: edit y=1\nlogin: winpid=4242\nlogin: typed"),
            4242
        );
        assert_eq!(parse_winpid("login: form NOT found"), 0);
    }
}
