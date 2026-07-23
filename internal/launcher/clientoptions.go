package launcher

import (
	"accountchanger/internal/vault"
	"os"
	"path/filepath"
	"sort"
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

var minimalOptions = map[string]string{
	"renderDistance":   "2",
	"maxFps":           "5",
	"particles":        "2",
	"fancyGraphics":    "false",
	"ao":               "0",
	"renderClouds":     "false",
	"mipmapLevels":     "0",
	"entityShadows":    "false",
	"enableVsync":      "false",
	"biomeBlendRadius": "0",
	"fboEnable":        "true",
	"useVbo":           "true",
}

var minimalOptionsOf = map[string]string{
	"ofFastRender":        "true",
	"ofFastMath":          "true",
	"ofClouds":            "3",
	"ofTrees":             "1",
	"ofDroppedItems":      "0",
	"ofRain":              "3",
	"ofBetterGrass":       "1",
	"ofConnectedTextures": "1",
	"ofWeather":           "false",
	"ofSky":               "false",
	"ofStars":             "false",
	"ofSunMoon":           "false",
	"ofVignette":          "1",
	"ofChunkUpdates":      "1",
	"ofSmoothFps":         "false",
	"ofFogType":           "3",
	"ofDynamicLights":     "0",
	"ofDynamicFov":        "false",
	"ofNaturalTextures":   "false",
	"ofEmissiveTextures":  "false",
	"ofCustomSky":         "false",
	"ofCustomColors":      "false",
	"ofBetterSnow":        "false",
	"ofClearWater":        "false",
	"ofAaLevel":           "0",
	"ofAfLevel":           "1",
	"ofShowFps":           "false",
}

func applyMinimalOptions(clientDir string) {
	updateOptionsFile(filepath.Join(clientDir, "options.txt"), minimalOptions)
	of := map[string]string{}
	for k, v := range minimalOptionsOf {
		of[k] = v
	}
	for _, k := range animationKeys {
		of[k] = "0"
	}
	updateOptionsFile(filepath.Join(clientDir, "optionsof.txt"), of)
}

func applyMinimalOptionsAll(updates string) {
	for _, c := range ListClients(updates) {
		applyMinimalOptions(filepath.Join(updates, c))
	}
}

const MinimalProfileName = "Минимальные настройки"

func minimalProfileText(m map[string]string) string {
	keys := make([]string, 0, len(m))
	for k := range m {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	var b strings.Builder
	for _, k := range keys {
		b.WriteString(k)
		b.WriteByte(':')
		b.WriteString(m[k])
		b.WriteByte('\n')
	}
	return b.String()
}

func MinimalProfileContent() map[string]string {
	of := map[string]string{}
	for k, v := range minimalOptionsOf {
		of[k] = v
	}
	for _, k := range animationKeys {
		of[k] = "0"
	}
	return map[string]string{
		"options.txt":    minimalProfileText(minimalOptions),
		"optionsof.txt":  minimalProfileText(of),
		"binds.json":     "",
		"voicechat.json": "",
	}
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
