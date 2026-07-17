package platform

import (
	"os"
	"path/filepath"
)

func dataDir() string {
	return filepath.Join(localAppData(), "AccountChanger")
}

func localAppData() string {
	if v := os.Getenv("LOCALAPPDATA"); v != "" {
		return v
	}
	home, _ := os.UserHomeDir()
	return filepath.Join(home, "AppData", "Local")
}
