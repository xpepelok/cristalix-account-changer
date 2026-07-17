package main

import (
	"encoding/json"
	"net/http"
	"strconv"
	"strings"
	"sync"
	"time"
)

type importItem struct {
	Login  string `json:"login"`
	Status string `json:"status"`
	Text   string `json:"text"`
	Name   string `json:"name,omitempty"`
}

type credPair struct {
	Login    string
	Password string
}

type importTask struct {
	item     *importItem
	password string
}

type importJob struct {
	mu      sync.Mutex
	running bool
	items   []*importItem
	cancel  chan struct{}
	updated time.Time
}

func (s *Server) savedValidLogins() map[string]bool {
	out := map[string]bool{}
	now := time.Now().Unix()
	for _, acc := range s.vault.List() {
		if acc.Name == "" || acc.Token == "" {
			continue
		}
		expired := acc.Expires > 0 && acc.Expires < now
		if !expired {
			out[strings.ToLower(acc.Name)] = true
		}
	}
	return out
}

func (s *Server) startImport(accounts []credPair) (bool, string) {
	if !credentialImportSupported {
		return false, errNoUIAutomation.Error()
	}
	s.imp.mu.Lock()
	if s.imp.running {
		s.imp.mu.Unlock()
		return false, "импорт уже идёт"
	}

	valid := s.savedValidLogins()
	seen := map[string]bool{}
	items := make([]*importItem, 0, len(accounts))
	tasks := make([]importTask, 0, len(accounts))
	dupes, skipped := 0, 0
	for _, a := range accounts {
		login := strings.TrimSpace(a.Login)
		if login == "" || a.Password == "" {
			continue
		}
		key := strings.ToLower(login)
		if seen[key] {
			dupes++
			continue
		}
		seen[key] = true
		if valid[key] {
			items = append(items, &importItem{Login: login, Status: "skip", Text: "уже сохранён"})
			skipped++
			continue
		}
		it := &importItem{Login: login, Status: "pending", Text: "в очереди"}
		items = append(items, it)
		tasks = append(tasks, importTask{item: it, password: a.Password})
	}

	if len(tasks) == 0 {
		s.imp.mu.Unlock()
		switch {
		case skipped > 0 && dupes > 0:
			return false, "нечего импортировать: уже сохранено " + itoa(skipped) + ", дублей " + itoa(dupes)
		case skipped > 0:
			return false, "все аккаунты уже сохранены (" + itoa(skipped) + ")"
		case dupes > 0:
			return false, "в списке только дубликаты"
		default:
			return false, "список пуст"
		}
	}

	s.imp.running = true
	s.imp.items = items
	s.imp.cancel = make(chan struct{})
	s.imp.updated = time.Now()
	s.imp.mu.Unlock()

	go s.runImport(tasks)
	return true, ""
}

func (s *Server) runImport(tasks []importTask) {
	defer func() {
		s.finishImport()
		s.imp.mu.Lock()
		s.imp.running = false
		s.imp.updated = time.Now()
		s.imp.mu.Unlock()
	}()
	for i := range tasks {
		if s.importCanceled() {
			s.markRestCanceled()
			return
		}
		s.setItem(tasks[i].item, "working", "вход…", "")
		name, err := s.importByCredentials(tasks[i].item.Login, tasks[i].password)
		tasks[i].password = ""
		if s.importCanceled() {
			if err == nil && name != "" {
				s.setItem(tasks[i].item, "ok", "✓ "+name, name)
			}
			s.markRestCanceled()
			return
		}
		if err != nil {
			s.setItem(tasks[i].item, "err", err.Error(), "")
		} else {
			s.setItem(tasks[i].item, "ok", "✓ "+name, name)
		}
	}
}

func (s *Server) setItem(it *importItem, status, text, name string) {
	s.imp.mu.Lock()
	it.Status, it.Text = status, text
	if name != "" {
		it.Name = name
	}
	s.imp.updated = time.Now()
	s.imp.mu.Unlock()
}

func (s *Server) markRestCanceled() {
	s.imp.mu.Lock()
	for _, it := range s.imp.items {
		if it.Status == "pending" || it.Status == "working" {
			it.Status = "canceled"
			it.Text = "отменено"
		}
	}
	s.imp.updated = time.Now()
	s.imp.mu.Unlock()
}

func (s *Server) importCanceled() bool {
	s.imp.mu.Lock()
	c := s.imp.cancel
	s.imp.mu.Unlock()
	if c == nil {
		return false
	}
	select {
	case <-c:
		return true
	default:
		return false
	}
}

func (s *Server) cancelImport() {
	s.imp.mu.Lock()
	if s.imp.running && s.imp.cancel != nil {
		select {
		case <-s.imp.cancel:
		default:
			close(s.imp.cancel)
		}
	}
	s.imp.mu.Unlock()

	s.importMu.Lock()
	p := s.importProc
	s.importProc = nil
	s.importMu.Unlock()
	if p != nil {
		_ = p.Kill()
	}
}

func (s *Server) importSnapshot() map[string]any {
	s.imp.mu.Lock()
	defer s.imp.mu.Unlock()
	items := make([]importItem, len(s.imp.items))
	ok, fail, skip, done := 0, 0, 0, 0
	for i, it := range s.imp.items {
		items[i] = *it
		switch it.Status {
		case "ok":
			ok++
			done++
		case "err":
			fail++
			done++
		case "skip":
			skip++
			done++
		case "canceled":
			done++
		}
	}
	return map[string]any{
		"running": s.imp.running,
		"total":   len(s.imp.items),
		"done":    done,
		"ok":      ok,
		"fail":    fail,
		"skip":    skip,
		"items":   items,
	}
}

func itoa(n int) string { return strconv.Itoa(n) }

func (s *Server) handleImportStart(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	var body struct {
		Accounts []struct {
			Login    string `json:"login"`
			Password string `json:"password"`
		} `json:"accounts"`
	}
	if err := json.NewDecoder(r.Body).Decode(&body); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]any{"ok": false, "error": "bad request"})
		return
	}
	accounts := make([]credPair, 0, len(body.Accounts))
	for _, a := range body.Accounts {
		accounts = append(accounts, credPair{Login: a.Login, Password: a.Password})
	}
	ok, reason := s.startImport(accounts)
	writeJSON(w, http.StatusOK, map[string]any{"ok": ok, "error": reason})
}

func (s *Server) handleImportProgress(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, http.StatusOK, s.importSnapshot())
}

func (s *Server) handleImportCancel(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	s.cancelImport()
	writeJSON(w, http.StatusOK, map[string]bool{"ok": true})
}
