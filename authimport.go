package main

import (
	"accountchanger/internal/launcher"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"
)

func (s *Server) importLog(format string, args ...any) {
	line := fmt.Sprintf("["+time.Now().Format("15:04:05")+"] "+format+"\n", args...)
	f, err := os.OpenFile(filepath.Join(s.paths.Data, "import.log"), os.O_CREATE|os.O_WRONLY|os.O_APPEND, 0o644)
	if err != nil {
		return
	}
	defer f.Close()
	_, _ = f.WriteString(line)
}

func (s *Server) waitVaultToken(before map[string]bool, timeout time.Duration) string {
	deadline := time.Now().Add(timeout)
	for time.Now().Before(deadline) {
		if s.importCanceled() {
			return ""
		}
		now := time.Now().Unix()
		for _, acc := range s.vault.List() {
			if acc.Name == "" || acc.Token == "" {
				continue
			}
			if acc.Expires > 0 && acc.Expires < now {
				continue
			}
			if !before[strings.ToLower(acc.Name)] {
				return acc.Name
			}
		}
		time.Sleep(150 * time.Millisecond)
	}
	return ""
}

func (s *Server) launcherAccountsDump() string {
	cfg, err := launcher.ReadLauncherConfig(s.paths.LauncherCfg)
	if err != nil {
		return "<read error: " + err.Error() + ">"
	}
	accs := launcher.LauncherAccounts(cfg)
	if len(accs) == 0 {
		return "(no accounts)"
	}
	parts := make([]string, 0, len(accs))
	for n, tok := range accs {
		parts = append(parts, fmt.Sprintf("%s(token:%v)", n, tok != ""))
	}
	return strings.Join(parts, ", ")
}
