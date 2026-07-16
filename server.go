package main

import (
	"accountchanger/internal/config"
	"accountchanger/internal/launcher"
	"accountchanger/internal/platform"
	"accountchanger/internal/player"
	"accountchanger/internal/update"
	"accountchanger/internal/vault"
	"embed"
	"encoding/json"
	"io"
	"io/fs"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"
)

//go:embed web
var webFiles embed.FS

//go:embed assets/icon.png
var iconBytes []byte

type Server struct {
	paths   platform.Paths
	vault   *vault.Vault
	watcher *launcher.Watcher
	queue   *launcher.LaunchQueue
	tracker *launcher.GameTracker
	logs    *launcher.LogStore
	cfg     *config.ConfigStore
	quit    chan struct{}
	restart func()
	focusMu sync.Mutex
	focus   func()

	importMu   sync.Mutex
	importProc *os.Process
	imp        importJob
}

func (s *Server) setFocus(f func()) {
	s.focusMu.Lock()
	s.focus = f
	s.focusMu.Unlock()
}

type accountDTO struct {
	UUID           string `json:"uuid"`
	Name           string `json:"name"`
	Launchable     bool   `json:"launchable"`
	Expired        bool   `json:"expired"`
	Expires        int64  `json:"expires"`
	LastLaunched   int64  `json:"lastLaunched"`
	Running        bool   `json:"running"`
	Launching      bool   `json:"launching"`
	Label          string `json:"label"`
	Pinned         bool   `json:"pinned"`
	Client         string `json:"client"`
	Profile        string `json:"profile"`
	Ram            int    `json:"ram"`
	MinGraphics    bool   `json:"minGraphics"`
	Fullscreen     bool   `json:"fullscreen"`
	DiscordRPC     bool   `json:"discordRPC"`
	AutoEnter      bool   `json:"autoEnter"`
	DebugMode      bool   `json:"debugMode"`
	RenderDistance int    `json:"renderDistance"`
	MaxFps         int    `json:"maxFps"`
	Animations     int    `json:"animations"`
	FastRender     int    `json:"fastRender"`
	FirstSeen      int64  `json:"firstSeen"`
}

func (s *Server) handler() http.Handler {
	mux := http.NewServeMux()

	sub, _ := fs.Sub(webFiles, "web")

	noCache := func(h http.Handler) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			w.Header().Set("Cache-Control", "no-cache, no-store, must-revalidate")
			w.Header().Set("Pragma", "no-cache")
			w.Header().Set("Expires", "0")
			h.ServeHTTP(w, r)
		})
	}
	fileServer := noCache(http.FileServer(http.FS(sub)))
	mux.Handle("/assets/", fileServer)
	mux.Handle("/skinview3d.bundle.js", fileServer)
	mux.Handle("/app.js", fileServer)
	mux.Handle("/sound.js", fileServer)
	mux.Handle("/theme.css", fileServer)

	mux.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path != "/" {
			http.NotFound(w, r)
			return
		}
		data, err := webFiles.ReadFile("web/index.html")
		if err != nil {
			http.Error(w, "not found", http.StatusNotFound)
			return
		}
		w.Header().Set("Content-Type", "text/html; charset=utf-8")
		w.Header().Set("Cache-Control", "no-cache, no-store, must-revalidate")
		_, _ = w.Write(data)
	})

	mux.HandleFunc("/favicon.png", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "image/png")
		_, _ = w.Write(iconBytes)
	})

	mux.HandleFunc("/api/status", s.handleStatus)
	mux.HandleFunc("/api/accounts", s.handleAccounts)
	mux.HandleFunc("/api/launch", s.handleLaunch)
	mux.HandleFunc("/api/launch-guest", s.handleLaunchGuest)
	mux.HandleFunc("/api/accounts/import-start", s.handleImportStart)
	mux.HandleFunc("/api/accounts/import-progress", s.handleImportProgress)
	mux.HandleFunc("/api/accounts/import-cancel", s.handleImportCancel)
	mux.HandleFunc("/api/accounts/refresh-start", s.handleRefreshStart)
	mux.HandleFunc("/api/launcher", s.handleLauncher)
	mux.HandleFunc("/api/forget", s.handleForget)
	mux.HandleFunc("/api/label", s.handleLabel)
	mux.HandleFunc("/api/pin", s.handlePin)
	mux.HandleFunc("/api/clients", s.handleClients)
	mux.HandleFunc("/api/update", s.handleUpdate)
	mux.HandleFunc("/api/update/apply", s.handleUpdateApply)
	mux.HandleFunc("/api/update/progress", s.handleUpdateProgress)
	mux.HandleFunc("/api/profiles", s.handleProfiles)
	mux.HandleFunc("/api/profiles/save", s.handleProfileSave)
	mux.HandleFunc("/api/profiles/content", s.handleProfileContent)
	mux.HandleFunc("/api/profiles/update", s.handleProfileUpdate)
	mux.HandleFunc("/api/profiles/delete", s.handleProfileDelete)
	mux.HandleFunc("/api/account/profile", s.handleAccountProfile)
	mux.HandleFunc("/api/account/ram", s.handleAccountRam)
	mux.HandleFunc("/api/account/launch-settings", s.handleLaunchSettings)
	mux.HandleFunc("/api/groups", s.handleGroups)
	mux.HandleFunc("/api/groups/create", s.handleGroupCreate)
	mux.HandleFunc("/api/groups/delete", s.handleGroupDelete)
	mux.HandleFunc("/api/groups/rename", s.handleGroupRename)
	mux.HandleFunc("/api/groups/pin", s.handleGroupPin)
	mux.HandleFunc("/api/groups/reorder", s.handleGroupReorder)
	mux.HandleFunc("/api/groups/members", s.handleGroupMembers)
	mux.HandleFunc("/api/groups/profile", s.handleGroupProfile)
	mux.HandleFunc("/api/groups/launch", s.handleGroupLaunch)
	mux.HandleFunc("/api/groups/close-all", s.handleGroupCloseAll)
	mux.HandleFunc("/api/groups/launch/pause", s.handleGroupLaunchPause)
	mux.HandleFunc("/api/groups/launch/resume", s.handleGroupLaunchResume)
	mux.HandleFunc("/api/groups/launch/progress", s.handleGroupLaunchProgress)
	mux.HandleFunc("/api/settings", s.handleSettings)
	mux.HandleFunc("/api/settings/autostart", s.handleAutostart)
	mux.HandleFunc("/api/settings/launcher", s.handleSettingsLauncher)
	mux.HandleFunc("/api/settings/custom-launcher", s.handleSettingsCustomLauncher)
	mux.HandleFunc("/api/settings/autoplay", s.handleSettingsAutoPlay)
	mux.HandleFunc("/api/settings/stats", s.handleSettingsStats)
	mux.HandleFunc("/api/stats", s.handleStats)
	mux.HandleFunc("/api/logs", s.handleLogs)
	mux.HandleFunc("/api/logs/get", s.handleLogsGet)
	mux.HandleFunc("/api/logs/clear", s.handleLogsClear)
	mux.HandleFunc("/api/logs/session/delete", s.handleLogSessionDelete)
	mux.HandleFunc("/api/stop", s.handleStop)
	mux.HandleFunc("/api/quit", s.handleQuit)
	mux.HandleFunc("/api/focus", s.handleFocus)
	mux.HandleFunc("/api/player", s.handlePlayer)

	mux.HandleFunc("/skin/", s.proxyTexture("skin"))
	mux.HandleFunc("/cape/", s.proxyTexture("cape"))

	return mux
}

func writeJSON(w http.ResponseWriter, code int, body any) {
	w.Header().Set("Content-Type", "application/json; charset=utf-8")
	w.WriteHeader(code)
	_ = json.NewEncoder(w).Encode(body)
}

func (s *Server) handleStatus(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, http.StatusOK, map[string]any{
		"launcherReady": fileExists(s.paths.LauncherExe),
		"dataDir":       s.paths.Data,
	})
}

func (s *Server) handleAccounts(w http.ResponseWriter, r *http.Request) {
	now := time.Now().Unix()
	accounts := s.vault.List()
	out := make([]accountDTO, 0, len(accounts))
	running, launching := s.tracker.Resolve()
	for _, acc := range accounts {
		if acc.Name == "" || acc.Token == "" {
			continue
		}
		expired := acc.Expires > 0 && acc.Expires < now
		_, isRunning := running[acc.UUID]
		out = append(out, accountDTO{
			UUID:           acc.UUID,
			Name:           acc.Name,
			Launchable:     acc.Name != "" && acc.Token != "",
			Expired:        expired,
			Expires:        acc.Expires,
			LastLaunched:   acc.LastLaunched,
			Running:        isRunning,
			Launching:      !isRunning && launching[acc.UUID],
			Label:          acc.Label,
			Pinned:         acc.Pinned,
			Client:         acc.Client,
			Profile:        acc.Profile,
			Ram:            acc.Ram,
			MinGraphics:    acc.MinGraphics,
			Fullscreen:     acc.Fullscreen,
			DiscordRPC:     acc.DiscordRPC,
			AutoEnter:      acc.AutoEnter,
			DebugMode:      acc.DebugMode,
			RenderDistance: acc.RenderDistance,
			MaxFps:         acc.MaxFps,
			Animations:     acc.Animations,
			FastRender:     acc.FastRender,
			FirstSeen:      acc.FirstSeen,
		})
	}
	writeJSON(w, http.StatusOK, out)
}

type uuidBody struct {
	UUID string `json:"uuid"`
}

func (s *Server) handleLaunch(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	var body struct {
		UUID   string `json:"uuid"`
		Client string `json:"client"`
	}
	if err := json.NewDecoder(r.Body).Decode(&body); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "bad request"})
		return
	}

	s.watcher.Capture()

	acc, ok := s.vault.Get(body.UUID)
	if !ok || acc.Name == "" || acc.Token == "" {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "нет сохранённого токена для этого аккаунта"})
		return
	}
	if running, _ := s.tracker.Resolve(); running[acc.UUID] != 0 {
		writeJSON(w, http.StatusOK, map[string]bool{"ok": true})
		return
	}
	s.queue.Enqueue(acc.UUID, body.Client)
	writeJSON(w, http.StatusOK, map[string]bool{"ok": true})
}

func (s *Server) startChosenLauncher() error {
	switch s.cfg.Launcher() {
	case config.LauncherCustom:
		return launcher.StartLauncher(s.cfg.CustomLauncher())
	case config.LauncherJar:
		if err := launcher.EnsureLauncherFrom(s.paths.LauncherJar, launcher.JarLauncherURL); err != nil {
			return err
		}
		java := launcher.ResolveJava(s.paths.Cristalix)
		return launcher.StartLauncherJar(java, s.paths.LauncherJar, "", "", nil, nil, nil)
	case config.LauncherNew:
		if err := launcher.EnsureLauncherFrom(s.paths.StaffLauncherExe, launcher.StaffLauncherURL); err != nil {
			return err
		}
		return launcher.StartLauncher(s.paths.StaffLauncherExe)
	default:
		if err := launcher.EnsureLauncher(s.paths.LauncherExe); err != nil {
			return err
		}
		return launcher.StartLauncher(s.paths.LauncherExe)
	}
}

func (s *Server) handleLaunchGuest(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	s.watcher.Capture()
	if err := launcher.AnnulAccount(s.paths.LauncherCfg); err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": err.Error()})
		return
	}
	if err := s.startChosenLauncher(); err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": err.Error()})
		return
	}
	writeJSON(w, http.StatusOK, map[string]bool{"ok": true})
}

func (s *Server) handleLauncher(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	if err := s.startChosenLauncher(); err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": err.Error()})
		return
	}
	writeJSON(w, http.StatusOK, map[string]bool{"ok": true})
}

func (s *Server) handleLabel(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	var body struct {
		UUID  string `json:"uuid"`
		Label string `json:"label"`
	}
	if err := json.NewDecoder(r.Body).Decode(&body); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "bad request"})
		return
	}
	label := strings.TrimSpace(body.Label)
	if r := []rune(label); len(r) > 8 {
		label = string(r[:8])
	}
	s.vault.SetLabel(body.UUID, label)
	writeJSON(w, http.StatusOK, map[string]bool{"ok": true})
}

func (s *Server) handlePin(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	var body struct {
		UUID   string `json:"uuid"`
		Pinned bool   `json:"pinned"`
	}
	if err := json.NewDecoder(r.Body).Decode(&body); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "bad request"})
		return
	}
	s.vault.SetPinned(body.UUID, body.Pinned)
	writeJSON(w, http.StatusOK, map[string]bool{"ok": true})
}

func (s *Server) handleStop(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	var body uuidBody
	if err := json.NewDecoder(r.Body).Decode(&body); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "bad request"})
		return
	}
	s.queue.KillProc(body.UUID)
	running, _ := s.tracker.Resolve()
	closed := false
	if pid := running[body.UUID]; pid != 0 {
		closed = platform.CloseGame(pid)
	}
	s.tracker.Forget(body.UUID)
	writeJSON(w, http.StatusOK, map[string]any{"ok": true, "closed": closed})
}

func (s *Server) handleForget(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	var body uuidBody
	if err := json.NewDecoder(r.Body).Decode(&body); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "bad request"})
		return
	}
	s.vault.Forget(body.UUID)
	writeJSON(w, http.StatusOK, map[string]bool{"ok": true})
}

func (s *Server) handleQuit(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, http.StatusOK, map[string]bool{"ok": true})
	go func() {
		time.Sleep(200 * time.Millisecond)
		close(s.quit)
	}()
}

func (s *Server) clientDirFor(client string) string {
	if client == "" {
		client = launcher.CurrentClient(s.paths.LauncherCfg)
	}
	if client == "" {
		return ""
	}
	return filepath.Join(launcher.UpdatesDir(s.paths.LauncherCfg, s.paths.Updates), client)
}

func (s *Server) handleProfiles(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, http.StatusOK, map[string]any{
		"profiles": launcher.ListProfiles(s.paths.Profiles),
		"files":    launcher.ProfileFiles,
	})
}

func (s *Server) handleProfileSave(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	var body struct {
		Name   string `json:"name"`
		Client string `json:"client"`
	}
	if err := json.NewDecoder(r.Body).Decode(&body); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "bad request"})
		return
	}
	name := launcher.SanitizeProfileName(body.Name)
	if name == "" {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "введите название профиля"})
		return
	}
	clientDir := s.clientDirFor(body.Client)
	if clientDir == "" {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "не найден клиент для чтения настроек"})
		return
	}
	copied, err := launcher.SaveProfileFromClient(s.paths.Profiles, name, clientDir)
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": err.Error()})
		return
	}
	if copied == 0 {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "в клиенте нет файлов настроек для сохранения"})
		return
	}
	writeJSON(w, http.StatusOK, map[string]any{"ok": true, "name": name, "copied": copied})
}

func (s *Server) handleProfileContent(w http.ResponseWriter, r *http.Request) {
	name := launcher.SanitizeProfileName(r.URL.Query().Get("name"))
	if name == "" {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "no name"})
		return
	}
	writeJSON(w, http.StatusOK, launcher.ReadProfile(s.paths.Profiles, name))
}

func (s *Server) handleProfileUpdate(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	var body struct {
		Name  string            `json:"name"`
		Files map[string]string `json:"files"`
	}
	if err := json.NewDecoder(r.Body).Decode(&body); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "bad request"})
		return
	}
	name := launcher.SanitizeProfileName(body.Name)
	if name == "" {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "no name"})
		return
	}
	if err := launcher.WriteProfile(s.paths.Profiles, name, body.Files); err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": err.Error()})
		return
	}
	writeJSON(w, http.StatusOK, map[string]bool{"ok": true})
}

func (s *Server) handleProfileDelete(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	var body struct {
		Name string `json:"name"`
	}
	if err := json.NewDecoder(r.Body).Decode(&body); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "bad request"})
		return
	}
	name := launcher.SanitizeProfileName(body.Name)
	if name != "" {
		_ = launcher.DeleteProfile(s.paths.Profiles, name)
	}
	writeJSON(w, http.StatusOK, map[string]bool{"ok": true})
}

func (s *Server) handleAccountProfile(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	var body struct {
		UUID    string `json:"uuid"`
		Profile string `json:"profile"`
	}
	if err := json.NewDecoder(r.Body).Decode(&body); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "bad request"})
		return
	}
	s.vault.SetProfile(body.UUID, launcher.SanitizeProfileName(body.Profile))
	writeJSON(w, http.StatusOK, map[string]bool{"ok": true})
}

func (s *Server) handleAccountRam(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	var body struct {
		UUIDs []string `json:"uuids"`
		Ram   int      `json:"ram"`
	}
	if err := json.NewDecoder(r.Body).Decode(&body); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "bad request"})
		return
	}
	ram := body.Ram
	if ram != 0 && ram < 1024 {
		ram = 1024
	}
	s.vault.SetRam(body.UUIDs, ram)
	writeJSON(w, http.StatusOK, map[string]bool{"ok": true})
}

func (s *Server) handleLaunchSettings(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	var body struct {
		UUIDs          []string `json:"uuids"`
		Ram            int      `json:"ram"`
		MinGraphics    bool     `json:"minGraphics"`
		Fullscreen     bool     `json:"fullscreen"`
		DiscordRPC     bool     `json:"discordRPC"`
		AutoEnter      bool     `json:"autoEnter"`
		DebugMode      bool     `json:"debugMode"`
		RenderDistance int      `json:"renderDistance"`
		MaxFps         int      `json:"maxFps"`
		Animations     int      `json:"animations"`
		FastRender     int      `json:"fastRender"`
	}
	if err := json.NewDecoder(r.Body).Decode(&body); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "bad request"})
		return
	}
	ram := body.Ram
	if ram != 0 && ram < 1024 {
		ram = 1024
	}
	s.vault.SetLaunchSettings(body.UUIDs, vault.LaunchOpts{
		Ram:            ram,
		MinGraphics:    body.MinGraphics,
		Fullscreen:     body.Fullscreen,
		DiscordRPC:     body.DiscordRPC,
		AutoEnter:      body.AutoEnter,
		DebugMode:      body.DebugMode,
		RenderDistance: body.RenderDistance,
		MaxFps:         body.MaxFps,
		Animations:     body.Animations,
		FastRender:     body.FastRender,
	})
	writeJSON(w, http.StatusOK, map[string]bool{"ok": true})
}

func (s *Server) handleUpdate(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, http.StatusOK, update.CheckUpdate())
}

func (s *Server) handleUpdateApply(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	info := update.CheckUpdate()
	if !info.Available {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "нет доступного обновления"})
		return
	}
	update.StartUpdate(func() {
		if s.restart != nil {
			s.restart()
		}
	})
	writeJSON(w, http.StatusOK, map[string]bool{"started": true})
}

func (s *Server) handleUpdateProgress(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, http.StatusOK, update.GetUpdateProgress())
}

func (s *Server) handleClients(w http.ResponseWriter, r *http.Request) {
	updates := launcher.UpdatesDir(s.paths.LauncherCfg, s.paths.Updates)
	writeJSON(w, http.StatusOK, map[string]any{
		"clients": launcher.ListClients(updates),
		"current": launcher.CurrentClient(s.paths.LauncherCfg),
	})
}

func (s *Server) handlePlayer(w http.ResponseWriter, r *http.Request) {
	name := r.URL.Query().Get("name")
	if name == "" {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "no name"})
		return
	}
	info := player.FetchPlayerInfo(name)
	if info == nil {
		writeJSON(w, http.StatusOK, map[string]any{})
		return
	}
	writeJSON(w, http.StatusOK, info)
}

func (s *Server) handleFocus(w http.ResponseWriter, r *http.Request) {
	s.focusMu.Lock()
	f := s.focus
	s.focusMu.Unlock()
	if f != nil {
		f()
	}
	writeJSON(w, http.StatusOK, map[string]bool{"ok": true})
}

var textureClient = &http.Client{Timeout: 15 * time.Second}

func (s *Server) proxyTexture(kind string) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		uuid := r.URL.Path[len("/"+kind+"/"):]
		if !launcher.LooksLikeUUID(uuid) {
			http.Error(w, "bad uuid", http.StatusBadRequest)
			return
		}
		upstream := "https://webdata.c7x.dev/textures/" + kind + "/" + uuid
		req, _ := http.NewRequest(http.MethodGet, upstream, nil)
		req.Header.Set("Origin", "https://cristalix.gg")
		req.Header.Set("Referer", "https://cristalix.gg/")
		resp, err := textureClient.Do(req)
		if err != nil {
			http.Error(w, "upstream error", http.StatusBadGateway)
			return
		}
		defer resp.Body.Close()
		w.Header().Set("Access-Control-Allow-Origin", "*")
		if resp.StatusCode != http.StatusOK {
			http.Error(w, "not found", http.StatusNotFound)
			return
		}
		w.Header().Set("Content-Type", "image/png")
		w.Header().Set("Cache-Control", "public, max-age=300")
		_, _ = io.Copy(w, resp.Body)
	}
}

func fileExists(p string) bool {
	_, err := os.Stat(p)
	return err == nil
}
