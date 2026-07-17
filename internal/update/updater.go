package update

import (
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"sync"
	"time"
)

const AppVersion = "0.3.2"
const releaseAPI = "https://api.github.com/repos/xpepelok/cristalix-account-changer/releases/latest"

type UpdateInfo struct {
	Current     string `json:"current"`
	Latest      string `json:"latest"`
	Available   bool   `json:"available"`
	Notes       string `json:"notes"`
	Name        string `json:"name"`
	PublishedAt string `json:"publishedAt"`
	assetURL    string
}

var updateMu sync.Mutex
var updateCache *UpdateInfo
var updateFetched time.Time

type UpdateProgress struct {
	Active     bool   `json:"active"`
	Done       bool   `json:"done"`
	Downloaded int64  `json:"downloaded"`
	Total      int64  `json:"total"`
	Percent    int    `json:"percent"`
	Error      string `json:"error"`
}

var progMu sync.Mutex
var progState UpdateProgress

func GetUpdateProgress() UpdateProgress {
	progMu.Lock()
	defer progMu.Unlock()
	p := progState
	if p.Total > 0 {
		p.Percent = int(p.Downloaded * 100 / p.Total)
	}
	return p
}

func setUpdateProgress(f func(*UpdateProgress)) {
	progMu.Lock()
	defer progMu.Unlock()
	f(&progState)
}

type progressReader struct {
	inner io.Reader
}

func (p *progressReader) Read(b []byte) (int, error) {
	n, err := p.inner.Read(b)
	if n > 0 {
		setUpdateProgress(func(u *UpdateProgress) { u.Downloaded += int64(n) })
	}
	return n, err
}

func StartUpdate(done func()) {
	progMu.Lock()
	if progState.Active {
		progMu.Unlock()
		return
	}
	progState = UpdateProgress{Active: true}
	progMu.Unlock()

	go func() {
		err := applyUpdate()
		if err != nil {
			setUpdateProgress(func(u *UpdateProgress) {
				u.Active = false
				u.Error = err.Error()
			})
			return
		}
		setUpdateProgress(func(u *UpdateProgress) {
			u.Active = false
			u.Done = true
		})
		time.Sleep(600 * time.Millisecond)
		restartApp()
		if done != nil {
			done()
		}
	}()
}

func CheckUpdate() *UpdateInfo {
	updateMu.Lock()
	if updateCache != nil && time.Since(updateFetched) < 10*time.Minute {
		cached := *updateCache
		updateMu.Unlock()
		return &cached
	}
	updateMu.Unlock()

	info := &UpdateInfo{Current: AppVersion, Latest: AppVersion}

	req, err := http.NewRequest(http.MethodGet, releaseAPI, nil)
	if err != nil {
		return info
	}
	req.Header.Set("Accept", "application/vnd.github+json")
	req.Header.Set("User-Agent", "AccountChanger")

	client := &http.Client{Timeout: 15 * time.Second}
	resp, err := client.Do(req)
	if err != nil {
		return info
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		return info
	}
	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return info
	}

	var rel struct {
		TagName     string `json:"tag_name"`
		Name        string `json:"name"`
		Body        string `json:"body"`
		PublishedAt string `json:"published_at"`
		Assets      []struct {
			Name string `json:"name"`
			URL  string `json:"browser_download_url"`
		} `json:"assets"`
	}
	if json.Unmarshal(body, &rel) != nil {
		return info
	}

	latest := extractVersion(rel.TagName)
	if latest == "" {
		return info
	}
	info.Latest = latest
	info.Notes = strings.TrimSpace(rel.Body)
	info.Name = strings.TrimSpace(rel.Name)
	info.PublishedAt = rel.PublishedAt
	for _, a := range rel.Assets {
		if assetMatches(a.Name) {
			info.assetURL = a.URL
			break
		}
	}
	info.Available = info.assetURL != "" && compareVersions(latest, AppVersion) > 0

	updateMu.Lock()
	updateCache = info
	updateFetched = time.Now()
	updateMu.Unlock()

	cached := *info
	return &cached
}

func compareVersions(a, b string) int {
	pa := strings.Split(a, ".")
	pb := strings.Split(b, ".")
	n := len(pa)
	if len(pb) > n {
		n = len(pb)
	}
	for i := 0; i < n; i++ {
		x, y := 0, 0
		if i < len(pa) {
			x, _ = strconv.Atoi(numPrefix(pa[i]))
		}
		if i < len(pb) {
			y, _ = strconv.Atoi(numPrefix(pb[i]))
		}
		if x != y {
			if x > y {
				return 1
			}
			return -1
		}
	}
	return 0
}

func extractVersion(tag string) string {
	start := -1
	for i := 0; i < len(tag); i++ {
		if tag[i] >= '0' && tag[i] <= '9' {
			start = i
			break
		}
	}
	if start < 0 {
		return ""
	}
	end := start
	for end < len(tag) {
		c := tag[end]
		if (c >= '0' && c <= '9') || c == '.' {
			end++
			continue
		}
		break
	}
	return strings.Trim(tag[start:end], ".")
}

func numPrefix(s string) string {
	out := ""
	for _, c := range s {
		if c < '0' || c > '9' {
			break
		}
		out += string(c)
	}
	if out == "" {
		return "0"
	}
	return out
}

func applyUpdate() error {
	info := CheckUpdate()
	if !info.Available || info.assetURL == "" {
		return errors.New("нет доступного обновления")
	}

	exePath, err := os.Executable()
	if err != nil {
		return err
	}
	newPath := exePath + ".new"
	oldPath := exePath + ".old"

	client := &http.Client{Timeout: 10 * time.Minute}
	req, err := http.NewRequest(http.MethodGet, info.assetURL, nil)
	if err != nil {
		return err
	}
	req.Header.Set("User-Agent", "AccountChanger")
	resp, err := client.Do(req)
	if err != nil {
		return err
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("не удалось скачать обновление (код %d)", resp.StatusCode)
	}

	setUpdateProgress(func(u *UpdateProgress) { u.Total = resp.ContentLength })

	f, err := os.Create(newPath)
	if err != nil {
		return err
	}
	if _, err := io.Copy(f, &progressReader{inner: resp.Body}); err != nil {
		f.Close()
		os.Remove(newPath)
		return err
	}
	f.Close()

	if err := finalizeBinary(newPath); err != nil {
		os.Remove(newPath)
		return err
	}

	os.Remove(oldPath)
	if err := os.Rename(exePath, oldPath); err != nil {
		os.Remove(newPath)
		return err
	}
	if err := os.Rename(newPath, exePath); err != nil {
		os.Rename(oldPath, exePath)
		return err
	}
	return nil
}

func CleanupOldExe() {
	if exePath, err := os.Executable(); err == nil {
		os.Remove(exePath + ".old")
	}
}

func restartApp() {
	exePath, err := os.Executable()
	if err != nil {
		return
	}
	cmd := exec.Command(exePath, "--updated")
	cmd.Dir = filepath.Dir(exePath)
	cmd.Start()
}
