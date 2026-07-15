package launcher

import (
	"os"
	"path/filepath"
)

func ClearWebCache(profile string) {
	if profile == "" {
		return
	}
	base := filepath.Join(profile, "EBWebView", "Default")
	for _, d := range []string{"Cache", "Code Cache", "GPUCache", "Service Worker"} {
		_ = os.RemoveAll(filepath.Join(base, d))
	}
}
