package app

import (
	"accountchanger/internal/launcher"
	"accountchanger/internal/vault"
	"errors"
	"net/http"
	"time"
)

const refreshWindow = 7 * 24 * time.Hour

type refreshTask struct {
	item *importItem
	uuid string
}

func (s *Server) expiringAccounts() []*vault.Account {
	now := time.Now().Unix()
	threshold := now + int64(refreshWindow/time.Second)
	var out []*vault.Account
	for _, acc := range s.vault.List() {
		if acc.Token == "" || acc.Expires <= 0 {
			continue
		}
		if acc.Expires <= threshold {
			out = append(out, acc)
		}
	}
	return out
}

func (s *Server) startRefreshExpiring() (bool, string) {
	if !credentialImportSupported {
		return false, errNoUIAutomation.Error()
	}
	s.imp.mu.Lock()
	if s.imp.running {
		s.imp.mu.Unlock()
		return false, "Процесс уже идёт"
	}
	accs := s.expiringAccounts()
	if len(accs) == 0 {
		s.imp.mu.Unlock()
		return false, "Нет токенов, истекающих в ближайшую неделю"
	}
	items := make([]*importItem, 0, len(accs))
	tasks := make([]refreshTask, 0, len(accs))
	for _, a := range accs {
		it := &importItem{Login: a.Name, Status: "pending", Text: "в очереди"}
		items = append(items, it)
		tasks = append(tasks, refreshTask{item: it, uuid: a.UUID})
	}
	s.imp.running = true
	s.imp.items = items
	s.imp.cancel = make(chan struct{})
	s.imp.updated = time.Now()
	s.imp.mu.Unlock()

	go s.runRefresh(tasks)
	return true, ""
}

func (s *Server) runRefresh(tasks []refreshTask) {
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
		s.setItem(tasks[i].item, "working", "обновление…", "")
		name, changed, err := s.refreshToken(tasks[i].uuid)
		if s.importCanceled() {
			if err == nil && changed {
				s.setItem(tasks[i].item, "ok", "✓ "+name, name)
			}
			s.markRestCanceled()
			return
		}
		switch {
		case err != nil:
			s.setItem(tasks[i].item, "err", err.Error(), "")
		case !changed:
			s.setItem(tasks[i].item, "skip", "ещё рано - токен в силе", name)
		default:
			s.setItem(tasks[i].item, "ok", "✓ "+name, name)
		}
	}
}

func (s *Server) refreshToken(uuid string) (string, bool, error) {
	acc, ok := s.vault.Get(uuid)
	if !ok || acc.Name == "" || acc.Token == "" {
		return "", false, errors.New("нет сохранённого токена")
	}
	name := acc.Name
	oldToken := acc.Token
	client := acc.Client
	if client == "" {
		client = launcher.CurrentClient(s.paths.LauncherCfg)
	}
	opts := launcher.AccountLaunchOpts(acc)
	opts.AutoEnter = false

	s.importLog("=== refresh '%s' ===", name)
	if err := s.ensureImportLauncher(func() {
		_ = launcher.ApplyAccount(s.paths.LauncherCfg, name, oldToken, client, opts)
	}, true); err != nil {
		s.importLog("refresh '%s': %v", name, err)
		return "", false, err
	}
	if s.importCanceled() {
		return "", false, errors.New("отменено")
	}

	s.importMu.Lock()
	p := s.importProc
	s.importMu.Unlock()
	pid := 0
	if p != nil {
		pid = p.Pid
	}
	s.importLog("refresh '%s': clicking ИГРАТЬ (pid %d) to mint a fresh token", name, pid)
	go launcher.ClickPlayButtonForPid(pid, 25)
	refreshed := s.waitRefreshedToken(uuid, oldToken, 35*time.Second)
	s.importMu.Lock()
	if s.importProc == p {
		s.importProc = nil
	}
	s.importMu.Unlock()
	if p != nil {
		s.importLog("refresh '%s': tearing down launcher+game (pid %d)", name, p.Pid)
		killProcessTree(p.Pid)
	}
	if !refreshed {
		s.importLog("refresh '%s': token unchanged - launcher kept it (not near enough to expiry)", name)
		return name, false, nil
	}
	s.importLog("refresh '%s': token refreshed ✓", name)
	return name, true, nil
}

func (s *Server) waitRefreshedToken(uuid, oldToken string, timeout time.Duration) bool {
	deadline := time.Now().Add(timeout)
	for time.Now().Before(deadline) {
		if s.importCanceled() {
			return false
		}
		if acc, ok := s.vault.Get(uuid); ok && acc.Token != "" && acc.Token != oldToken {
			return true
		}
		time.Sleep(150 * time.Millisecond)
	}
	return false
}

func (s *Server) handleRefreshStart(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	ok, reason := s.startRefreshExpiring()
	writeJSON(w, http.StatusOK, map[string]any{"ok": ok, "error": reason})
}
