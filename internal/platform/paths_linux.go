package platform

import (
	"os"
	"path/filepath"
)

func dataDir() string {
	return filepath.Join(xdgDataHome(), "AccountChanger")
}

func xdgDataHome() string {
	if v := os.Getenv("XDG_DATA_HOME"); filepath.IsAbs(v) {
		return v
	}
	home, _ := os.UserHomeDir()
	return filepath.Join(home, ".local", "share")
}

func xdgConfigHome() string {
	if v := os.Getenv("XDG_CONFIG_HOME"); filepath.IsAbs(v) {
		return v
	}
	home, _ := os.UserHomeDir()
	return filepath.Join(home, ".config")
}
