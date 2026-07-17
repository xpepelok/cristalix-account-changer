package platform

import (
	"errors"
	"os"
	"path/filepath"
	"strings"
)

const autostartFile = "accountchanger.desktop"

func autostartPath() string {
	return filepath.Join(xdgConfigHome(), "autostart", autostartFile)
}

func AutostartEnabled() bool {
	raw, err := os.ReadFile(autostartPath())
	if err != nil {
		return false
	}

	for _, line := range strings.Split(string(raw), "\n") {
		if strings.EqualFold(strings.TrimSpace(line), "Hidden=true") {
			return false
		}
	}
	return true
}

func SetAutostart(enabled bool) error {
	path := autostartPath()
	if !enabled {
		if err := os.Remove(path); err != nil && !errors.Is(err, os.ErrNotExist) {
			return err
		}
		return nil
	}
	exe, err := os.Executable()
	if err != nil {
		return err
	}
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return err
	}
	entry := "[Desktop Entry]\n" +
		"Type=Application\n" +
		"Name=AccountChanger\n" +
		"Comment=Менеджер аккаунтов Cristalix\n" +
		"Exec=" + quoteExec(exe) + "\n" +
		"Terminal=false\n" +
		"X-GNOME-Autostart-enabled=true\n"
	return os.WriteFile(path, []byte(entry), 0o644)
}

func quoteExec(path string) string {
	if !strings.ContainsAny(path, ` "'\`+"`"+`$`) {
		return path
	}
	r := strings.NewReplacer(`\`, `\\`, `"`, `\"`, "`", "\\`", `$`, `\$`)
	return `"` + r.Replace(path) + `"`
}
