package launcher

import (
	"accountchanger/internal/config"
	"accountchanger/internal/platform"
	"accountchanger/internal/vault"
	"time"
)

const defaultGameTitle = "Cristalix"

var titleKick = make(chan struct{}, 1)

func kickTitles() {
	select {
	case titleKick <- struct{}{}:
	default:
	}
}

func TitleLoop(t *GameTracker, v *vault.Vault, cfg *config.ConfigStore) {
	renamed := map[uint32]bool{}
	for {
		select {
		case <-time.After(3 * time.Second):
		case <-titleKick:
			time.Sleep(400 * time.Millisecond)
		}
		running, _ := t.Resolve()
		if !cfg.WindowTitle() {
			if len(renamed) == 0 {
				continue
			}
			live := map[uint32]bool{}
			for _, pid := range running {
				live[pid] = true
			}
			for pid := range renamed {
				if live[pid] {
					platform.SetWindowTitleForPid(pid, defaultGameTitle)
				}
			}
			renamed = map[uint32]bool{}
			continue
		}
		for uuid, pid := range running {
			acc, ok := v.Get(uuid)
			if !ok || acc.Name == "" {
				continue
			}
			allowGameTitle(acc.Name)
			if platform.SetWindowTitleForPid(pid, acc.Name) {
				renamed[pid] = true
			}
		}
	}
}
