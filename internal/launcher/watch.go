package launcher

import (
	"accountchanger/internal/jwt"
	"accountchanger/internal/platform"
	"accountchanger/internal/vault"
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
	cfg, err := ReadLauncherConfig(w.Paths.LauncherCfg)
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
		w.Vault.UpsertToken(name, token, claims)
	}
}
