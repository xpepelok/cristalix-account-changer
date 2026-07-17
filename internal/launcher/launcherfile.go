package launcher

import (
	"accountchanger/internal/vault"
	"encoding/json"
	"os"
)

func ReadLauncherConfig(path string) (map[string]any, error) {
	raw, err := readFileShareAll(path)
	if err != nil {
		if os.IsNotExist(err) {
			return map[string]any{}, nil
		}
		return nil, err
	}
	cfg := map[string]any{}
	if err := json.Unmarshal(raw, &cfg); err != nil {
		return map[string]any{}, nil
	}
	return cfg, nil
}

func writeLauncherConfig(path string, cfg map[string]any) error {
	payload, err := json.Marshal(cfg)
	if err != nil {
		return err
	}
	tmp := path + ".tmp"
	if err := os.WriteFile(tmp, payload, 0o644); err != nil {
		return err
	}
	return os.Rename(tmp, path)
}

func LauncherAccounts(cfg map[string]any) map[string]string {
	out := map[string]string{}
	raw, ok := cfg["accounts"].(map[string]any)
	if !ok {
		return out
	}
	for name, token := range raw {
		if s, ok := token.(string); ok {
			out[name] = s
		}
	}
	return out
}

func ApplyAccount(path, name, token, client string, opts vault.LaunchOpts) error {
	cfg, err := ReadLauncherConfig(path)
	if err != nil {
		return err
	}
	cfg["accounts"] = map[string]any{name: token}
	cfg["currentAccount"] = name
	if client != "" {
		cfg["lastClient"] = client
	}
	cfg["autoEnter"] = opts.AutoEnter
	cfg["minimalGraphics"] = opts.MinGraphics
	cfg["fullscreen"] = opts.Fullscreen
	cfg["discordRPC"] = opts.DiscordRPC
	cfg["debugMode"] = opts.DebugMode
	mem := -1
	if opts.Ram >= 512 {
		mem = opts.Ram
	}
	cfg["memoryAmount"] = mem
	return writeLauncherConfig(path, cfg)
}

func AnnulAccount(path string) error {
	cfg, err := ReadLauncherConfig(path)
	if err != nil {
		return err
	}
	cfg["accounts"] = map[string]any{}
	cfg["currentAccount"] = ""
	cfg["autoEnter"] = false
	return writeLauncherConfig(path, cfg)
}
