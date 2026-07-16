package config

import (
	"encoding/json"
	"os"
	"sync"
)

const (
	LauncherNormal = "normal"
	LauncherJar    = "jar"
	LauncherNew    = "new"
	LauncherCustom = "custom"
)

type AppConfig struct {
	Launcher       string `json:"launcher"`
	CustomLauncher string `json:"customLauncher,omitempty"`
	AutoPlay       *bool  `json:"autoPlay,omitempty"`
	Stats          *bool  `json:"stats,omitempty"`
}

type ConfigStore struct {
	mu   sync.Mutex
	path string
	cfg  AppConfig
}

func OpenConfig(path string) *ConfigStore {
	def := true
	defStats := true
	c := &ConfigStore{path: path, cfg: AppConfig{Launcher: LauncherJar, AutoPlay: &def, Stats: &defStats}}
	if data, err := os.ReadFile(path); err == nil {
		var stored AppConfig
		if json.Unmarshal(data, &stored) == nil {
			c.cfg.CustomLauncher = stored.CustomLauncher
			if validLauncher(stored.Launcher) && (stored.Launcher != LauncherCustom || stored.CustomLauncher != "") {
				c.cfg.Launcher = stored.Launcher
			}
			if stored.AutoPlay != nil {
				*c.cfg.AutoPlay = *stored.AutoPlay
			}
			if stored.Stats != nil {
				*c.cfg.Stats = *stored.Stats
			}
		}
	}
	return c
}

func (c *ConfigStore) Stats() bool {
	c.mu.Lock()
	defer c.mu.Unlock()
	return c.cfg.Stats != nil && *c.cfg.Stats
}

func (c *ConfigStore) SetStats(v bool) {
	c.mu.Lock()
	defer c.mu.Unlock()
	*c.cfg.Stats = v
	c.save()
}

func (c *ConfigStore) AutoPlay() bool {
	c.mu.Lock()
	defer c.mu.Unlock()
	return c.cfg.AutoPlay != nil && *c.cfg.AutoPlay
}

func (c *ConfigStore) SetAutoPlay(v bool) {
	c.mu.Lock()
	defer c.mu.Unlock()
	*c.cfg.AutoPlay = v
	c.save()
}

func (c *ConfigStore) save() {
	data, err := json.Marshal(c.cfg)
	if err != nil {
		return
	}
	tmp := c.path + ".tmp"
	if os.WriteFile(tmp, data, 0o644) == nil {
		_ = os.Rename(tmp, c.path)
	}
}

func validLauncher(v string) bool {
	return v == LauncherNormal || v == LauncherJar || v == LauncherNew || v == LauncherCustom
}

func (c *ConfigStore) Launcher() string {
	c.mu.Lock()
	defer c.mu.Unlock()
	return c.cfg.Launcher
}

func (c *ConfigStore) CustomLauncher() string {
	c.mu.Lock()
	defer c.mu.Unlock()
	return c.cfg.CustomLauncher
}

func (c *ConfigStore) SetLauncher(v string) bool {
	if !validLauncher(v) {
		return false
	}
	c.mu.Lock()
	defer c.mu.Unlock()
	if v == LauncherCustom && c.cfg.CustomLauncher == "" {
		return false
	}
	c.cfg.Launcher = v
	c.save()
	return true
}

func (c *ConfigStore) SetCustomLauncher(path string) bool {
	if path == "" {
		return false
	}
	c.mu.Lock()
	defer c.mu.Unlock()
	c.cfg.CustomLauncher = path
	c.cfg.Launcher = LauncherCustom
	c.save()
	return true
}
