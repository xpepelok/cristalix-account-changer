package main

import (
	"accountchanger/internal/launcher"
	"accountchanger/internal/platform"
	"errors"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"syscall"
	"time"
)

func killProcessTree(pid int) {
	cmd := exec.Command("taskkill", "/F", "/T", "/PID", strconv.Itoa(pid))
	cmd.Env = launcher.CleanEnv()
	cmd.SysProcAttr = &syscall.SysProcAttr{CreationFlags: launcher.CreateNoWindow}
	_ = cmd.Run()
}

func (s *Server) importLog(format string, args ...any) {
	line := fmt.Sprintf("["+time.Now().Format("15:04:05")+"] "+format+"\n", args...)
	f, err := os.OpenFile(filepath.Join(s.paths.Data, "import.log"), os.O_CREATE|os.O_WRONLY|os.O_APPEND, 0o644)
	if err != nil {
		return
	}
	defer f.Close()
	_, _ = f.WriteString(line)
}

func startJarProc(java, jar string) (*os.Process, error) {
	if _, err := os.Stat(jar); err != nil {
		return nil, err
	}
	cmd := exec.Command(java, "-jar", jar)
	cmd.Dir = filepath.Dir(jar)
	cmd.Env = launcher.CleanEnv()
	cmd.SysProcAttr = &syscall.SysProcAttr{CreationFlags: launcher.CreateNoWindow}
	if err := cmd.Start(); err != nil {
		return nil, err
	}
	return cmd.Process, nil
}

func (s *Server) ensureImportLauncher(prepare func(), forceFresh bool) error {
	s.importMu.Lock()
	proc := s.importProc
	s.importMu.Unlock()
	if !forceFresh && proc != nil && platform.WindowForPid(uint32(proc.Pid)) != 0 {
		s.importLog("launcher: reusing running instance (pid %d)", proc.Pid)
		return nil
	}
	if proc != nil {
		s.importLog("launcher: previous instance gone, restarting")
		killProcessTree(proc.Pid)
		s.importMu.Lock()
		if s.importProc == proc {
			s.importProc = nil
		}
		s.importMu.Unlock()
	}
	if prepare != nil {
		prepare()
	} else {
		_ = launcher.AnnulAccount(s.paths.LauncherCfg)
	}

	platform.ClearStaleLocks(s.paths.Cristalix)
	if err := launcher.EnsureLauncherFrom(s.paths.LauncherJar, launcher.JarLauncherURL); err != nil {
		s.importLog("launcher: ensure jar failed: %v", err)
		return errors.New("не удалось подготовить лаунчер: " + err.Error())
	}
	java := launcher.ResolveJava(s.paths.Cristalix)
	s.importLog("launcher: starting jar (java=%s)", java)
	newProc, err := startJarProc(java, s.paths.LauncherJar)
	if err != nil {
		s.importLog("launcher: start failed: %v", err)
		return errors.New("не удалось запустить лаунчер: " + err.Error())
	}
	s.importMu.Lock()
	s.importProc = newProc
	s.importMu.Unlock()

	deadline := time.Now().Add(45 * time.Second)
	appeared := false
	for time.Now().Before(deadline) {
		if s.importCanceled() {
			return errors.New("отменено")
		}
		if platform.WindowForPid(uint32(newProc.Pid)) != 0 {
			appeared = true
			break
		}
		time.Sleep(400 * time.Millisecond)
	}
	if !appeared {
		s.importLog("launcher: window did not appear within 45s (pid %d)", newProc.Pid)
		_ = newProc.Kill()
		s.importMu.Lock()
		if s.importProc == newProc {
			s.importProc = nil
		}
		s.importMu.Unlock()
		return errors.New("окно лаунчера не появилось")
	}
	s.importLog("launcher: window appeared (pid %d), waiting to settle", newProc.Pid)
	time.Sleep(1000 * time.Millisecond)
	return nil
}

func (s *Server) importByCredentials(login, password string) (string, error) {
	login = strings.TrimSpace(login)
	if login == "" || password == "" {
		return "", errors.New("укажите логин и пароль")
	}

	s.importLog("=== import '%s' ===", login)
	if err := s.ensureImportLauncher(nil, false); err != nil {
		s.importLog("import '%s': %v", login, err)
		return "", err
	}
	if s.importCanceled() {
		return "", errors.New("отменено")
	}

	before := s.savedValidLogins()
	loginCode, loginOut := launcher.UiaLogin(login, password, 35)
	s.importLog("launcher.UiaLogin exit=%d | %s", loginCode, loginOut)
	switch loginCode {
	case 1:
		return "", errors.New("окно входа не найдено (форма авторизации не появилась)")
	case 2:
		return "", errors.New("не удалось сфокусироваться на окне лаунчера")
	case 3:
		return "", errors.New("ошибка ввода логина/пароля")
	case 4:
		return "", errors.New("неправильный логин или пароль")
	}

	s.importMu.Lock()
	p := s.importProc
	s.importMu.Unlock()
	pid := 0
	if p != nil {
		pid = p.Pid
	}
	s.importLog("import '%s': clicking ИГРАТЬ (pid %d) to flush the token into .launcher", login, pid)
	go launcher.ClickPlayButtonForPid(pid, 25)
	name := s.waitVaultToken(before, 35*time.Second)
	s.importMu.Lock()
	if s.importProc == p {
		s.importProc = nil
	}
	s.importMu.Unlock()
	if p != nil {
		s.importLog("import '%s': tearing down launcher+game (pid %d)", login, p.Pid)
		killProcessTree(p.Pid)
	}
	if name == "" {
		s.importLog("import '%s': token did NOT appear (final: %s)", login, s.launcherAccountsDump())
		return "", errors.New("вход не удался или превышено время ожидания (проверь логин/пароль, возможна капча/2FA)")
	}
	s.importLog("import '%s': token captured as account '%s' ✓", login, name)
	return name, nil
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

func (s *Server) finishImport() {
	s.importMu.Lock()
	defer s.importMu.Unlock()
	if s.importProc != nil {
		s.importLog("finish: closing launcher+game (pid %d)", s.importProc.Pid)
		killProcessTree(s.importProc.Pid)
		s.importProc = nil
	}
	_ = launcher.AnnulAccount(s.paths.LauncherCfg)
}
