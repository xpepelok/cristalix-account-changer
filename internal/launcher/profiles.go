package launcher

import (
	"os"
	"path/filepath"
	"sort"
	"strings"
)

var ProfileFiles = []string{"binds.json", "options.txt", "optionsof.txt", "voicechat.json"}

func SanitizeProfileName(name string) string {
	name = strings.TrimSpace(name)
	name = strings.Map(func(r rune) rune {
		switch r {
		case '/', '\\', ':', '*', '?', '"', '<', '>', '|', '.':
			return '_'
		}
		return r
	}, name)
	if len(name) > 40 {
		name = name[:40]
	}
	return strings.TrimSpace(name)
}

func ListProfiles(profilesDir string) []string {
	entries, err := os.ReadDir(profilesDir)
	if err != nil {
		return nil
	}
	var out []string
	for _, e := range entries {
		if e.IsDir() {
			out = append(out, e.Name())
		}
	}
	sort.Strings(out)
	return out
}

func SaveProfileFromClient(profilesDir, name, clientDir string) (int, error) {
	dest := filepath.Join(profilesDir, name)
	if err := os.MkdirAll(dest, 0o755); err != nil {
		return 0, err
	}
	copied := 0
	for _, f := range ProfileFiles {
		data, err := os.ReadFile(filepath.Join(clientDir, f))
		if err != nil {
			continue
		}
		if os.WriteFile(filepath.Join(dest, f), data, 0o644) == nil {
			copied++
		}
	}
	return copied, nil
}

func ReadProfile(profilesDir, name string) map[string]string {
	out := map[string]string{}
	for _, f := range ProfileFiles {
		data, err := os.ReadFile(filepath.Join(profilesDir, name, f))
		if err == nil {
			out[f] = string(data)
		} else {
			out[f] = ""
		}
	}
	return out
}

func WriteProfile(profilesDir, name string, files map[string]string) error {
	dest := filepath.Join(profilesDir, name)
	if err := os.MkdirAll(dest, 0o755); err != nil {
		return err
	}
	for _, f := range ProfileFiles {
		content, ok := files[f]
		if !ok {
			continue
		}
		if err := os.WriteFile(filepath.Join(dest, f), []byte(content), 0o644); err != nil {
			return err
		}
	}
	return nil
}

func applyProfile(profilesDir, name, clientDir string) {
	base := filepath.Join(profilesDir, name)
	for _, f := range ProfileFiles {
		data, err := os.ReadFile(filepath.Join(base, f))
		if err != nil {
			continue
		}
		_ = os.WriteFile(filepath.Join(clientDir, f), data, 0o644)
	}
}

func DeleteProfile(profilesDir, name string) error {
	return os.RemoveAll(filepath.Join(profilesDir, name))
}
