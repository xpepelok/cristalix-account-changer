package app

import (
	"accountchanger/internal/launcher"
	"os"
	"syscall"
)

func killProcessTree(pid int) {
	if pid <= 0 {
		return
	}

	if err := syscall.Kill(-pid, syscall.SIGKILL); err == nil {
		return
	}
	if p, err := os.FindProcess(pid); err == nil {
		_ = p.Kill()
	}
}

func (s *Server) ensureImportLauncher(prepare func(), forceFresh bool) error {
	return errNoUIAutomation
}

func (s *Server) importAll(tasks []importTask) {
	for i := range tasks {
		if s.importCanceled() {
			s.markRestCanceled()
			return
		}
		s.setItem(tasks[i].item, "err", errNoUIAutomation.Error(), "")
		tasks[i].password = ""
	}
}

func (s *Server) importByCredentials(login, password string) (string, error) {
	return "", errNoUIAutomation
}

func (s *Server) finishImport() {
	s.importMu.Lock()
	defer s.importMu.Unlock()
	if s.importProc != nil {
		killProcessTree(s.importProc.Pid)
		s.importProc = nil
	}
	_ = launcher.AnnulAccount(s.paths.LauncherCfg)
}
