package platform

import (
	"os"

	"golang.org/x/sys/windows/registry"
)

const autostartPath = `Software\Microsoft\Windows\CurrentVersion\Run`
const autostartName = "AccountChanger"

func AutostartEnabled() bool {
	k, err := registry.OpenKey(registry.CURRENT_USER, autostartPath, registry.QUERY_VALUE)
	if err != nil {
		return false
	}
	defer k.Close()
	v, _, err := k.GetStringValue(autostartName)
	if err != nil {
		return false
	}
	return v != ""
}

func SetAutostart(enabled bool) error {
	k, _, err := registry.CreateKey(registry.CURRENT_USER, autostartPath, registry.SET_VALUE)
	if err != nil {
		return err
	}
	defer k.Close()
	if !enabled {
		if err := k.DeleteValue(autostartName); err != nil && err != registry.ErrNotExist {
			return err
		}
		return nil
	}
	exe, err := os.Executable()
	if err != nil {
		return err
	}
	return k.SetStringValue(autostartName, `"`+exe+`"`)
}
