package platform

import (
	"os"
	"path/filepath"
	"strconv"
)

func ClearStaleLocks(cristalixDir string) {
	locksDir := filepath.Join(cristalixDir, "locks")
	entries, err := os.ReadDir(locksDir)
	if err != nil {
		return
	}
	open, ok := openFilePaths()
	if !ok {
		return
	}
	real := locksDir
	if resolved, err := filepath.EvalSymlinks(locksDir); err == nil {
		real = resolved
	}
	for _, e := range entries {
		if e.IsDir() {
			continue
		}
		if open[filepath.Join(real, e.Name())] {
			continue
		}
		os.Remove(filepath.Join(locksDir, e.Name()))
	}
}

func openFilePaths() (map[string]bool, bool) {
	procDir, err := os.Open("/proc")
	if err != nil {
		return nil, false
	}
	defer procDir.Close()
	names, err := procDir.Readdirnames(-1)
	if err != nil {
		return nil, false
	}

	out := map[string]bool{}
	for _, name := range names {
		if _, err := strconv.Atoi(name); err != nil {
			continue
		}
		fdDir := filepath.Join("/proc", name, "fd")
		fds, err := os.ReadDir(fdDir)
		if err != nil {
			continue
		}
		for _, fd := range fds {
			target, err := os.Readlink(filepath.Join(fdDir, fd.Name()))
			if err != nil || !filepath.IsAbs(target) {
				continue
			}
			out[target] = true
		}
	}
	return out, true
}
