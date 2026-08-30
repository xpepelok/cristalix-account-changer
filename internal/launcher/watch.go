package launcher

import (
	"accountchanger/internal/jwt"
	"accountchanger/internal/platform"
	"accountchanger/internal/vault"
	"os"
	"path/filepath"
	"time"
)

type Watcher struct {
	Paths platform.Paths
	Vault *vault.Vault
}

func (w *Watcher) Run() {
	w.Capture()
	go w.watchEvents()

	ticker := time.NewTicker(3 * time.Second)
	defer ticker.Stop()
	for range ticker.C {
		w.Capture()
	}
}

func (w *Watcher) Capture() {
	w.captureFile(w.Paths.LauncherCfg)
	for _, cfg := range InstanceConfigs(w.Paths.Data) {
		w.captureFile(cfg)
	}
}

func (w *Watcher) captureFile(path string) {
	CaptureConfig(w.Vault, path)
}

func InstanceConfigs(dataDir string) []string {
	base := filepath.Join(dataDir, "instances")
	entries, err := os.ReadDir(base)
	if err != nil {
		return nil
	}
	out := make([]string, 0, len(entries))
	for _, e := range entries {
		if !e.IsDir() {
			continue
		}
		out = append(out, filepath.Join(base, e.Name(), ".cristalix", ".launcher"))
	}
	return out
}

func CaptureConfig(v *vault.Vault, path string) {
	if v == nil {
		return
	}
	cfg, err := ReadLauncherConfig(path)
	if err != nil {
		return
	}
	for name, token := range LauncherAccounts(cfg) {
		if token == "" {
			continue
		}
		claims, err := jwt.Parse(token)
		if err != nil {
			continue
		}
		v.UpsertToken(name, token, claims)
	}
}
