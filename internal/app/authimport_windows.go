package app

import (
	"accountchanger/internal/launcher"
	"accountchanger/internal/platform"
	"errors"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"syscall"
	"time"
)

func killProcessTree(pid int) {
	if pid <= 0 {
		return
	}
	for _, p := range launcher.ProcessTreePids(uint32(pid)) {
		cmd := exec.Command("taskkill", "/F", "/PID", strconv.Itoa(int(p)))
		cmd.Env = launcher.CleanEnv()
		cmd.SysProcAttr = &syscall.SysProcAttr{CreationFlags: launcher.CreateNoWindow}
		_ = cmd.Run()
	}
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
	java, ok := launcher.UsableJava(s.paths.Cristalix)
	if !ok {
		s.importLog("launcher: no suitable Java found (need 11+); refusing to start jar")
		return errors.New("не найдена подходящая Java для лаунчера - запусти клиент Cristalix хотя бы раз (он скачает свою Java), затем повтори импорт")
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
	s.importLog("launcher: starting jar (java=%s)", java)
	newProc, err := startJarProc(java, s.paths.LauncherJar)
	if err != nil {
		s.importLog("launcher: start failed: %v", err)
		return errors.New("не удалось запустить лаунчер: " + err.Error())
	}
	s.importMu.Lock()
	s.importProc = newProc
	s.importMu.Unlock()

	deadline := time.Now().Add(120 * time.Second)
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
		s.importLog("launcher: window did not appear within 120s (pid %d)", newProc.Pid)
		_ = newProc.Kill()
		s.importMu.Lock()
		if s.importProc == newProc {
			s.importProc = nil
		}
		s.importMu.Unlock()
		return errors.New("окно лаунчера не появилось")
	}
	s.importLog("launcher: window appeared (pid %d), waiting to settle", newProc.Pid)
	time.Sleep(300 * time.Millisecond)
	return nil
}

func (s *Server) killImportLauncher() {
	s.importMu.Lock()
	p := s.importProc
	s.importProc = nil
	s.importMu.Unlock()
	if p != nil {
		killProcessTree(p.Pid)
	}
}

func (s *Server) importOneReuse(login, password string) (name string, usedPlay bool, winpid int, err error) {
	login = strings.TrimSpace(login)
	if login == "" || password == "" {
		return "", false, 0, errors.New("укажите логин и пароль")
	}
	before := s.savedValidLogins()
	backoff := 2 * time.Second
	authRetries := 0
	for {
		code, wp, out := launcher.UiaLogin(login, password, 90)
		if wp > 0 {
			winpid = wp
		}
		s.importLog("import '%s': UiaLogin exit=%d winpid=%d | %s", login, code, wp, out)
		switch code {
		case 1:
			return "", false, winpid, errors.New("окно входа не найдено (форма авторизации не появилась)")
		case 2:
			return "", false, winpid, errors.New("не удалось сфокусироваться на окне лаунчера")
		case 3:
			return "", false, winpid, errors.New("ошибка ввода логина/пароля")
		case 4:
			return "", false, winpid, errors.New("неправильный логин или пароль")
		case 5:
			if authRetries < 3 && !s.importCanceled() {
				authRetries++
				s.importLog("import '%s': auth-login, ретрай %d/3 через %s", login, authRetries, backoff)
				time.Sleep(backoff)
				backoff *= 2
				continue
			}
			return "", false, winpid, errors.New("Неизвестная ошибка: auth-login")
		}
		break
	}
	if name = s.waitVaultToken(before, 10*time.Second); name != "" {
		s.importLog("import '%s': токен на логине ✓", login)
		return name, false, winpid, nil
	}
	s.importMu.Lock()
	p := s.importProc
	s.importMu.Unlock()
	pid := 0
	if p != nil {
		pid = p.Pid
	}
	s.importLog("import '%s': нет токена на логине, жму ИГРАТЬ (pid %d)", login, pid)
	go launcher.ClickPlayButtonForPid(pid, 25)
	if name = s.waitVaultToken(before, 35*time.Second); name == "" {
		return "", true, winpid, errors.New("вход не удался или превышено время ожидания (проверь логин/пароль, возможна капча/2FA)")
	}
	return name, true, winpid, nil
}

func (s *Server) importAll(tasks []importTask) {
	reuseReady := false
	curWin := 0
	killCurrent := func() {
		if curWin > 0 {
			platform.CloseGame(uint32(curWin))
			curWin = 0
		}
		s.killImportLauncher()
	}
	for i := range tasks {
		if s.importCanceled() {
			killCurrent()
			s.markRestCanceled()
			return
		}
		if !reuseReady {
			if err := s.ensureImportLauncher(nil, true); err != nil {
				s.setItem(tasks[i].item, "err", err.Error(), "")
				tasks[i].password = ""
				continue
			}
		}
		reuseReady = false
		s.setItem(tasks[i].item, "working", "вход…", "")

		name, usedPlay, win, err := s.importOneReuse(tasks[i].item.Login, tasks[i].password)
		if win > 0 {
			curWin = win
		}
		if err != nil && importRetryable(err) && !s.importCanceled() {
			s.importLog("import '%s': повтор после ошибки: %v", tasks[i].item.Login, err)
			killCurrent()
			if e2 := s.ensureImportLauncher(nil, true); e2 == nil {
				s.setItem(tasks[i].item, "working", "повтор…", "")
				name, usedPlay, win, err = s.importOneReuse(tasks[i].item.Login, tasks[i].password)
				if win > 0 {
					curWin = win
				}
			}
		}
		tasks[i].password = ""

		if s.importCanceled() {
			if err == nil && name != "" {
				s.setItem(tasks[i].item, "ok", "✓ "+name, name)
			}
			killCurrent()
			s.markRestCanceled()
			return
		}
		if err != nil {
			s.setItem(tasks[i].item, "err", err.Error(), "")
		} else {
			s.setItem(tasks[i].item, "ok", "✓ "+name, name)
		}

		last := i == len(tasks)-1
		if err == nil && !usedPlay && !last {
			code, out := launcher.UiaLogout(25)
			s.importLog("import: logout exit=%d | %s", code, out)
			reuseReady = code == 0
		}
		if !reuseReady {
			killCurrent()
		}
	}
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
	loginCode, _, loginOut := launcher.UiaLogin(login, password, 90)
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
	name := s.waitVaultToken(before, 8*time.Second)
	if name == "" {
		s.importLog("import '%s': нет токена после логина, жму ИГРАТЬ (pid %d)", login, pid)
		go launcher.ClickPlayButtonForPid(pid, 25)
		name = s.waitVaultToken(before, 35*time.Second)
	} else {
		s.importLog("import '%s': токен получен ПОСЛЕ ЛОГИНА, без ИГРАТЬ ✓", login)
	}
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
