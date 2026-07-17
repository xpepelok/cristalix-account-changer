package platform

import (
	"os"
	"path/filepath"

	"golang.org/x/sys/windows"
)

func ClearStaleLocks(cristalixDir string) {
	locksDir := filepath.Join(cristalixDir, "locks")
	entries, err := os.ReadDir(locksDir)
	if err != nil {
		return
	}
	for _, e := range entries {
		if e.IsDir() {
			continue
		}
		path := filepath.Join(locksDir, e.Name())
		if lockFileIsStale(path) {
			os.Remove(path)
		}
	}
}

func lockFileIsStale(path string) bool {
	p, err := windows.UTF16PtrFromString(path)
	if err != nil {
		return false
	}
	handle, err := windows.CreateFile(
		p,
		windows.GENERIC_READ,
		0,
		nil,
		windows.OPEN_EXISTING,
		windows.FILE_ATTRIBUTE_NORMAL,
		0,
	)
	if err != nil {
		return false
	}
	windows.CloseHandle(handle)
	return true
}
