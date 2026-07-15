package launcher

import (
	"os"
	"path/filepath"
	"sort"
)

func UpdatesDir(cfgPath, fallback string) string {
	cfg, err := ReadLauncherConfig(cfgPath)
	if err == nil {
		if v, ok := cfg["updatesDirectory"].(string); ok && v != "" {
			return v
		}
	}
	return fallback
}

func ListClients(updates string) []string {
	entries, err := os.ReadDir(updates)
	if err != nil {
		return nil
	}
	var clients []string
	for _, e := range entries {
		if !e.IsDir() {
			continue
		}
		if _, err := os.Stat(filepath.Join(updates, e.Name(), "minecraft.jar")); err == nil {
			clients = append(clients, e.Name())
		}
	}
	sort.Strings(clients)
	return clients
}

func CurrentClient(cfgPath string) string {
	cfg, err := ReadLauncherConfig(cfgPath)
	if err != nil {
		return ""
	}
	if v, ok := cfg["lastClient"].(string); ok {
		return v
	}
	return ""
}
