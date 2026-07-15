package stats

import (
	"accountchanger/internal/config"
	"accountchanger/internal/platform"
	"accountchanger/internal/update"
	"crypto/rand"
	"encoding/hex"
	"encoding/json"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"
)

const statsPingURL = "https://stats.xpepelok.me/ping"
const statsInfoURL = "https://stats.xpepelok.me/stats"

func installID(dataDir string) string {
	p := filepath.Join(dataDir, "install.id")
	if b, err := os.ReadFile(p); err == nil {
		if s := strings.TrimSpace(string(b)); len(s) >= 8 {
			return s
		}
	}
	buf := make([]byte, 16)
	if _, err := rand.Read(buf); err != nil {
		return ""
	}
	id := hex.EncodeToString(buf)
	_ = os.WriteFile(p, []byte(id), 0o600)
	return id
}

func Loop(paths platform.Paths, cfg *config.ConfigStore) {
	sendStatsPing(paths, cfg)
	t := time.NewTicker(5 * time.Minute)
	defer t.Stop()
	for range t.C {
		sendStatsPing(paths, cfg)
	}
}

func sendStatsPing(paths platform.Paths, cfg *config.ConfigStore) {
	if !cfg.Stats() {
		return
	}
	id := installID(paths.Data)
	if id == "" {
		return
	}
	client := &http.Client{Timeout: 8 * time.Second}
	req, err := http.NewRequest(http.MethodGet, statsPingURL+"?id="+id+"&v="+update.AppVersion, nil)
	if err != nil {
		return
	}
	resp, err := client.Do(req)
	if err != nil {
		return
	}
	_, _ = io.Copy(io.Discard, resp.Body)
	_ = resp.Body.Close()
}

type Snap struct {
	Online int `json:"online"`
	Active int `json:"active"`
	Total  int `json:"total"`
}

var (
	statsMu   sync.Mutex
	statsData = Snap{Online: -1, Active: -1, Total: -1}
	statsAt   time.Time
)

func Fetch() Snap {
	statsMu.Lock()
	if statsData.Online >= 0 && time.Since(statsAt) < 45*time.Second {
		d := statsData
		statsMu.Unlock()
		return d
	}
	statsMu.Unlock()

	client := &http.Client{Timeout: 8 * time.Second}
	resp, err := client.Get(statsInfoURL)
	if err != nil {
		return statsSnapshot()
	}
	defer resp.Body.Close()
	var d Snap
	if json.NewDecoder(resp.Body).Decode(&d) != nil {
		return statsSnapshot()
	}
	statsMu.Lock()
	statsData = d
	statsAt = time.Now()
	statsMu.Unlock()
	return d
}

func statsSnapshot() Snap {
	statsMu.Lock()
	defer statsMu.Unlock()
	return statsData
}
