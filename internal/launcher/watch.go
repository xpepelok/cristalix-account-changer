package launcher

import (
	"accountchanger/internal/jwt"
	"accountchanger/internal/platform"
	"accountchanger/internal/vault"
	"path/filepath"
	"time"

	"golang.org/x/sys/windows"
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

func (w *Watcher) watchEvents() {
	dirPtr, err := windows.UTF16PtrFromString(filepath.Dir(w.Paths.LauncherCfg))
	if err != nil {
		return
	}
	for {
		h, err := windows.CreateFile(dirPtr, windows.FILE_LIST_DIRECTORY,
			windows.FILE_SHARE_READ|windows.FILE_SHARE_WRITE|windows.FILE_SHARE_DELETE,
			nil, windows.OPEN_EXISTING, windows.FILE_FLAG_BACKUP_SEMANTICS, 0)
		if err != nil {
			time.Sleep(2 * time.Second)
			continue
		}
		buf := make([]byte, 8192)
		for {
			var n uint32
			werr := windows.ReadDirectoryChanges(h, &buf[0], uint32(len(buf)), false,
				windows.FILE_NOTIFY_CHANGE_LAST_WRITE|windows.FILE_NOTIFY_CHANGE_SIZE|windows.FILE_NOTIFY_CHANGE_FILE_NAME,
				&n, nil, 0)
			if werr != nil {
				break
			}
			w.Capture()
		}
		windows.CloseHandle(h)
		time.Sleep(500 * time.Millisecond)
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
