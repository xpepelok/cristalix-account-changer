package launcher

import (
	"accountchanger/internal/vault"
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

var animationKeys = []string{
	"ofAnimatedWater", "ofAnimatedLava", "ofAnimatedFire", "ofAnimatedPortal",
	"ofAnimatedRedstone", "ofAnimatedExplosion", "ofAnimatedFlame", "ofAnimatedSmoke",
	"ofVoidParticles", "ofWaterParticles", "ofRainSplash", "ofPortalParticles",
	"ofPotionParticles", "ofFireworkParticles", "ofDrippingWaterLava",
	"ofAnimatedTerrain", "ofAnimatedTextures",
}

func updateOptionsFile(path string, updates map[string]string) {
	if len(updates) == 0 {
		return
	}
	var lines []string
	if data, err := os.ReadFile(path); err == nil {
		lines = strings.Split(strings.ReplaceAll(string(data), "\r\n", "\n"), "\n")
	} else if !os.IsNotExist(err) {
		return
	}
	seen := map[string]bool{}
	for i, line := range lines {
		idx := strings.IndexByte(line, ':')
		if idx < 0 {
			continue
		}
		key := line[:idx]
		if v, ok := updates[key]; ok {
			lines[i] = key + ":" + v
			seen[key] = true
		}
	}
	for key, v := range updates {
		if !seen[key] {
			lines = append(lines, key+":"+v)
		}
	}
	out := strings.Join(lines, "\n")
	_ = os.WriteFile(path, []byte(out), 0o644)
}

func applyClientOptionsAll(updates string, acc *vault.Account) {
	for _, c := range ListClients(updates) {
		applyClientOptions(filepath.Join(updates, c), acc)
	}
}

func applyClientOptions(clientDir string, acc *vault.Account) {
	opts := map[string]string{}
	if acc.RenderDistance > 0 {
		opts["renderDistance"] = strconv.Itoa(acc.RenderDistance)
	}
	if acc.MaxFps > 0 {
		opts["maxFps"] = strconv.Itoa(acc.MaxFps)
	}
	updateOptionsFile(filepath.Join(clientDir, "options.txt"), opts)

	of := map[string]string{}
	if acc.FastRender == 1 {
		of["ofFastRender"] = "false"
	} else if acc.FastRender == 2 {
		of["ofFastRender"] = "true"
	}
	if acc.Animations != 0 {
		val := "0"
		if acc.Animations == 2 {
			val = "2"
		}
		for _, k := range animationKeys {
			of[k] = val
		}
	}
	updateOptionsFile(filepath.Join(clientDir, "optionsof.txt"), of)
}
