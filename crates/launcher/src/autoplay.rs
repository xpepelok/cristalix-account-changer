use crate::games::Games;

pub const AUTO_PLAY_TIMEOUT: u32 = 180;

#[cfg(windows)]
const TITLE_SCRIPT: &str = r#"
$ProgressPreference = 'SilentlyContinue'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
$playLabels = @('ИГРАТЬ','PLAY','Play')
$titles = @({{TITLES}})
$btnCond = [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ControlTypeProperty, [Windows.Automation.ControlType]::Button)
$deadline = [DateTime]::UtcNow.AddSeconds({{TIMEOUT}})
while ([DateTime]::UtcNow -lt $deadline) {
  foreach ($p in Get-Process -ErrorAction SilentlyContinue) {
    if ($p.MainWindowHandle -eq [IntPtr]::Zero) { continue }
    if (-not ($titles -contains $p.MainWindowTitle)) { continue }
    try {
      $root = [Windows.Automation.AutomationElement]::FromHandle($p.MainWindowHandle)
      $btns = $root.FindAll([Windows.Automation.TreeScope]::Descendants, $btnCond)
      for ($i = 0; $i -lt $btns.Count; $i++) {
        $bn = $btns.Item($i)
        if (($playLabels -contains $bn.Current.Name) -and $bn.Current.IsEnabled) {
          try {
            $bn.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke()
            exit 0
          } catch {}
        }
      }
    } catch {}
  }
  Start-Sleep -Milliseconds 300
}
"#;

#[cfg(windows)]
const PID_SCRIPT: &str = r#"
$ProgressPreference = 'SilentlyContinue'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
$playLabels = @('ИГРАТЬ','PLAY','Play')
$btnCond = [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ControlTypeProperty, [Windows.Automation.ControlType]::Button)
$deadline = [DateTime]::UtcNow.AddSeconds({{TIMEOUT}})
while ([DateTime]::UtcNow -lt $deadline) {
  $p = Get-Process -Id {{PID}} -ErrorAction SilentlyContinue
  if ($p -and $p.MainWindowHandle -ne [IntPtr]::Zero) {
    try {
      $root = [Windows.Automation.AutomationElement]::FromHandle($p.MainWindowHandle)
      $btns = $root.FindAll([Windows.Automation.TreeScope]::Descendants, $btnCond)
      for ($i = 0; $i -lt $btns.Count; $i++) {
        $bn = $btns.Item($i)
        if (($playLabels -contains $bn.Current.Name) -and $bn.Current.IsEnabled) {
          $bn.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke()
          Write-Output "clicked play"; exit 0
        }
      }
    }
    catch {}
  }
  Start-Sleep -Milliseconds 100
}
Write-Output "play not found/clicked"; exit 1
"#;

#[cfg(windows)]
pub(crate) fn powershell(script: &str) -> tokio::process::Command {
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD;

    let bytes: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let mut command = crate::command::detached("powershell");
    command
        .arg("-NoProfile")
        .arg("-NonInteractive")
        .arg("-EncodedCommand")
        .arg(STANDARD.encode(bytes));
    command
}

#[cfg(windows)]
fn title_list(games: &Games) -> String {
    games
        .allowed_titles()
        .iter()
        .map(|title| format!("'{}'", title.replace('\'', "''")))
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(windows)]
pub async fn click_play_for_pid(games: &Games, pid: u32, timeout: u32) {
    let script = if pid == 0 {
        TITLE_SCRIPT
            .replacen("{{TIMEOUT}}", &timeout.to_string(), 1)
            .replacen("{{TITLES}}", &title_list(games), 1)
    } else {
        PID_SCRIPT
            .replacen("{{TIMEOUT}}", &timeout.to_string(), 1)
            .replacen("{{PID}}", &pid.to_string(), 1)
    };
    let _ = powershell(&script).status().await;
}

#[cfg(not(windows))]
pub async fn click_play_for_pid(_games: &Games, _pid: u32, _timeout: u32) {}
