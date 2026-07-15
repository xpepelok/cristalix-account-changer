package launcher

import (
	"encoding/base64"
	"os/exec"
	"strconv"
	"strings"
	"syscall"
	"unicode/utf16"
)

const autoPlayTimeout = 180

const autoPlayScript = `
$ProgressPreference = 'SilentlyContinue'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
$cond = [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty, 'ИГРАТЬ')
$deadline = [DateTime]::UtcNow.AddSeconds({{TIMEOUT}})
while ([DateTime]::UtcNow -lt $deadline) {
  foreach ($p in Get-Process -ErrorAction SilentlyContinue) {
    if ($p.MainWindowHandle -eq [IntPtr]::Zero) { continue }
    if ($p.MainWindowTitle -ne 'Cristalix') { continue }
    try {
      $root = [Windows.Automation.AutomationElement]::FromHandle($p.MainWindowHandle)
      $el = $root.FindFirst([Windows.Automation.TreeScope]::Descendants, $cond)
      if ($el -and $el.Current.ControlType -eq [Windows.Automation.ControlType]::Button -and $el.Current.IsEnabled) {
        $el.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke()
        exit 0
      }
    } catch {}
  }
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
	cmd.SysProcAttr = &syscall.SysProcAttr{CreationFlags: CreateNoWindow}
	_ = cmd.Run()
}

const autoPlayPidScript = `
$ProgressPreference = 'SilentlyContinue'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
$cond = [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty, 'ИГРАТЬ')
$deadline = [DateTime]::UtcNow.AddSeconds({{TIMEOUT}})
while ([DateTime]::UtcNow -lt $deadline) {
  $p = Get-Process -Id {{PID}} -ErrorAction SilentlyContinue
  if ($p -and $p.MainWindowHandle -ne [IntPtr]::Zero) {
    try {
      $root = [Windows.Automation.AutomationElement]::FromHandle($p.MainWindowHandle)
      $el = $root.FindFirst([Windows.Automation.TreeScope]::Descendants, $cond)
      if ($el -and $el.Current.ControlType -eq [Windows.Automation.ControlType]::Button -and $el.Current.IsEnabled) {
        $el.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke()
        Write-Output "clicked ИГРАТЬ"; exit 0
      }
    } catch {}
  }
  Start-Sleep -Milliseconds 100
}
Write-Output "ИГРАТЬ not found/clicked"; exit 1
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
	cmd.SysProcAttr = &syscall.SysProcAttr{CreationFlags: CreateNoWindow}
	_ = cmd.Run()
}
