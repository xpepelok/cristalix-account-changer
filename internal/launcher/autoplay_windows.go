package launcher

import (
	"encoding/base64"
	"os/exec"
	"strconv"
	"strings"
	"unicode/utf16"
)

const autoPlayTimeout = 180

const autoPlayScript = `
$ProgressPreference = 'SilentlyContinue'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
$playLabels = @('ИГРАТЬ','PLAY','Play')
$btnCond = [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ControlTypeProperty, [Windows.Automation.ControlType]::Button)
$deadline = [DateTime]::UtcNow.AddSeconds({{TIMEOUT}})
$clickedAny = $false
$quiet = 0
while ([DateTime]::UtcNow -lt $deadline) {
  $foundThisPass = $false
  foreach ($p in Get-Process -ErrorAction SilentlyContinue) {
    if ($p.MainWindowHandle -eq [IntPtr]::Zero) { continue }
    if ($p.MainWindowTitle -ne 'Cristalix') { continue }
    try {
      $root = [Windows.Automation.AutomationElement]::FromHandle($p.MainWindowHandle)
      $btns = $root.FindAll([Windows.Automation.TreeScope]::Descendants, $btnCond)
      for ($i = 0; $i -lt $btns.Count; $i++) {
        $bn = $btns.Item($i)
        if (($playLabels -contains $bn.Current.Name) -and $bn.Current.IsEnabled) {
          try { $bn.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke(); $clickedAny = $true; $foundThisPass = $true } catch {}
        }
      }
    } catch {}
  }
  if ($clickedAny -and -not $foundThisPass) { $quiet++; if ($quiet -ge 100) { exit 0 } } else { $quiet = 0 }
  Start-Sleep -Milliseconds 100
}
`

func encodePowershell(s string) string {
	u := utf16.Encode([]rune(s))
	b := make([]byte, len(u)*2)
	for i, r := range u {
		b[i*2] = byte(r)
		b[i*2+1] = byte(r >> 8)
	}
	return base64.StdEncoding.EncodeToString(b)
}

func clickPlayButton(timeoutSec int) {
	script := strings.Replace(autoPlayScript, "{{TIMEOUT}}", strconv.Itoa(timeoutSec), 1)
	cmd := exec.Command("powershell", "-NoProfile", "-NonInteractive", "-EncodedCommand", encodePowershell(script))
	cmd.Env = CleanEnv()
	detach(cmd)
	_ = cmd.Run()
}

const autoPlayPidScript = `
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
`

func ClickPlayButtonForPid(pid, timeoutSec int) {
	if pid <= 0 {
		clickPlayButton(timeoutSec)
		return
	}
	script := strings.Replace(autoPlayPidScript, "{{TIMEOUT}}", strconv.Itoa(timeoutSec), 1)
	script = strings.Replace(script, "{{PID}}", strconv.Itoa(pid), 1)
	cmd := exec.Command("powershell", "-NoProfile", "-NonInteractive", "-EncodedCommand", encodePowershell(script))
	cmd.Env = CleanEnv()
	detach(cmd)
	_ = cmd.Run()
}
