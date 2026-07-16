package main

import (
	"accountchanger/internal/launcher"
	"accountchanger/internal/platform"
	"accountchanger/internal/stats"
	"encoding/json"
	"net/http"
)

func decodeBody(r *http.Request, dst any) bool {
	return json.NewDecoder(r.Body).Decode(dst) == nil
}

func (s *Server) handleGroups(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, http.StatusOK, map[string]any{"groups": s.vault.SortedGroups()})
}

func (s *Server) handleGroupCreate(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	var body struct {
		Name string `json:"name"`
	}
	if !decodeBody(r, &body) {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "bad request"})
		return
	}
	g, err := s.vault.CreateGroup(body.Name)
	if err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": err.Error()})
		return
	}
	writeJSON(w, http.StatusOK, map[string]any{"group": g})
}

func (s *Server) handleGroupDelete(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	var body struct {
		ID string `json:"id"`
	}
	if !decodeBody(r, &body) {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "bad request"})
		return
	}
	s.vault.DeleteGroup(body.ID)
	writeJSON(w, http.StatusOK, map[string]bool{"ok": true})
}

func (s *Server) handleGroupRename(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	var body struct {
		ID   string `json:"id"`
		Name string `json:"name"`
	}
	if !decodeBody(r, &body) {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "bad request"})
		return
	}
	if err := s.vault.RenameGroup(body.ID, body.Name); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": err.Error()})
		return
	}
	writeJSON(w, http.StatusOK, map[string]bool{"ok": true})
}

func (s *Server) handleGroupPin(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	var body struct {
		ID     string `json:"id"`
		Pinned bool   `json:"pinned"`
	}
	if !decodeBody(r, &body) {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "bad request"})
		return
	}
	s.vault.SetGroupPinned(body.ID, body.Pinned)
	writeJSON(w, http.StatusOK, map[string]bool{"ok": true})
}

func (s *Server) handleGroupReorder(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	var body struct {
		IDs []string `json:"ids"`
	}
	if !decodeBody(r, &body) {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "bad request"})
		return
	}
	s.vault.ReorderGroups(body.IDs)
	writeJSON(w, http.StatusOK, map[string]bool{"ok": true})
}

func (s *Server) handleGroupMembers(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	var body struct {
		ID     string `json:"id"`
		Add    string `json:"add"`
		Remove string `json:"remove"`
	}
	if !decodeBody(r, &body) {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "bad request"})
		return
	}
	if body.Add != "" {
		s.vault.AddGroupMember(body.ID, body.Add)
	}
	if body.Remove != "" {
		s.vault.RemoveGroupMember(body.ID, body.Remove)
	}
	writeJSON(w, http.StatusOK, map[string]bool{"ok": true})
}

func (s *Server) handleGroupProfile(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	var body struct {
		ID      string `json:"id"`
		Profile string `json:"profile"`
	}
	if !decodeBody(r, &body) {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "bad request"})
		return
	}
	s.vault.SetGroupProfile(body.ID, body.Profile)
	writeJSON(w, http.StatusOK, map[string]bool{"ok": true})
}

func (s *Server) handleGroupLaunch(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	var body struct {
		ID string `json:"id"`
	}
	if !decodeBody(r, &body) {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "bad request"})
		return
	}
	members, profile, ok := s.vault.GroupMembers(body.ID)
	if !ok {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "группа не найдена"})
		return
	}
	if len(members) == 0 {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "в группе нет аккаунтов"})
		return
	}
	s.watcher.Capture()
	go s.queue.LaunchGroup(members, profile)
	writeJSON(w, http.StatusOK, map[string]bool{"started": true})
}

func (s *Server) handleSettings(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, http.StatusOK, map[string]any{"autostart": platform.AutostartEnabled(), "launcher": s.cfg.Launcher(), "customLauncher": s.cfg.CustomLauncher(), "autoPlay": s.cfg.AutoPlay(), "stats": s.cfg.Stats()})
}

func (s *Server) handleSettingsCustomLauncher(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	var body struct {
		Path string `json:"path"`
	}
	if !decodeBody(r, &body) {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "bad request"})
		return
	}
	if !s.cfg.SetCustomLauncher(body.Path) {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "не указан путь к лаунчеру"})
		return
	}
	writeJSON(w, http.StatusOK, map[string]any{"launcher": s.cfg.Launcher(), "customLauncher": s.cfg.CustomLauncher()})
}

func (s *Server) handleStats(w http.ResponseWriter, r *http.Request) {
	if !s.cfg.Stats() {
		writeJSON(w, http.StatusOK, map[string]any{"enabled": false, "online": -1, "active": -1, "total": -1})
		return
	}
	d := stats.Fetch()
	writeJSON(w, http.StatusOK, map[string]any{"enabled": true, "online": d.Online, "active": d.Active, "total": d.Total})
}

func (s *Server) handleSettingsStats(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	var body struct {
		Enabled bool `json:"enabled"`
	}
	if !decodeBody(r, &body) {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "bad request"})
		return
	}
	s.cfg.SetStats(body.Enabled)
	writeJSON(w, http.StatusOK, map[string]any{"stats": s.cfg.Stats()})
}

func (s *Server) handleSettingsAutoPlay(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	var body struct {
		Enabled bool `json:"enabled"`
	}
	if !decodeBody(r, &body) {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "bad request"})
		return
	}
	s.cfg.SetAutoPlay(body.Enabled)
	writeJSON(w, http.StatusOK, map[string]any{"autoPlay": s.cfg.AutoPlay()})
}

func (s *Server) handleSettingsLauncher(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	var body struct {
		Launcher string `json:"launcher"`
	}
	if !decodeBody(r, &body) {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "bad request"})
		return
	}
	if !s.cfg.SetLauncher(body.Launcher) {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "неизвестный лаунчер"})
		return
	}
	writeJSON(w, http.StatusOK, map[string]any{"launcher": s.cfg.Launcher()})
}

func (s *Server) handleAutostart(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	var body struct {
		Enabled bool `json:"enabled"`
	}
	if !decodeBody(r, &body) {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "bad request"})
		return
	}
	if err := platform.SetAutostart(body.Enabled); err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": err.Error()})
		return
	}
	writeJSON(w, http.StatusOK, map[string]any{"autostart": platform.AutostartEnabled()})
}

func (s *Server) handleLogs(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, http.StatusOK, map[string]any{"logs": s.logs.Summaries()})
}

func (s *Server) handleLogsGet(w http.ResponseWriter, r *http.Request) {
	uuid := r.URL.Query().Get("uuid")
	session := r.URL.Query().Get("session")
	lines, ok := s.logs.Get(uuid, session)
	if !ok {
		writeJSON(w, http.StatusOK, map[string]any{"lines": []launcher.LogLine{}, "sessions": []any{}})
		return
	}
	writeJSON(w, http.StatusOK, map[string]any{"lines": lines, "sessions": s.logs.Sessions(uuid)})
}

func (s *Server) handleLogsClear(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	if r.URL.Query().Get("info") == "1" {
		writeJSON(w, http.StatusOK, map[string]any{"bytes": s.logs.SizeAll()})
		return
	}
	s.logs.ClearAll()
	writeJSON(w, http.StatusOK, map[string]bool{"ok": true})
}

func (s *Server) handleLogSessionDelete(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	var body struct {
		UUID    string `json:"uuid"`
		Session string `json:"session"`
	}
	if !decodeBody(r, &body) {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "bad request"})
		return
	}
	s.logs.DeleteSession(body.UUID, body.Session)
	writeJSON(w, http.StatusOK, map[string]bool{"ok": true})
}
