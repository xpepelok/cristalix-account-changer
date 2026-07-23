package launcher

import (
	"accountchanger/internal/config"
	"accountchanger/internal/platform"
	"accountchanger/internal/player"
	"accountchanger/internal/vault"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"
)

const launchingWindow = 2 * time.Minute
const gameAppearWindow = 15 * time.Minute
const stagingClient = "Minigames-staging-java21"

func launchIsStaff(name string) bool {
	staff := player.CachedStaff(name)
	return staff != "" && !strings.EqualFold(staff, "PLAYER")
}

func shortUUID(uuid string) string {
	if len(uuid) > 8 {
		return uuid[:8]
	}
	return uuid
}

type LaunchQueue struct {
	paths          platform.Paths
	vault          *vault.Vault
	tracker        *GameTracker
	jobs           chan string
	mu             sync.Mutex
	pending        map[string]bool
	client         map[string]string
	order          []string
	current        string
	groupActive    bool
	groupPaused    bool
	groupCancelled bool
	groupDone      int
	groupTotal     int
	groupResumeCh  chan struct{}
	groupSkips     []GroupSkip
	logs           *LogStore
	cfg            *config.ConfigStore
	procMu         sync.Mutex
	procs          map[string]*os.Process
}

func NewLaunchQueue(paths platform.Paths, vault *vault.Vault, tracker *GameTracker, logs *LogStore, cfg *config.ConfigStore) *LaunchQueue {
	q := &LaunchQueue{
		paths:   paths,
		vault:   vault,
		tracker: tracker,
		jobs:    make(chan string, 32),
		pending: map[string]bool{},
		client:  map[string]string{},
		logs:    logs,
		cfg:     cfg,
		procs:   map[string]*os.Process{},
	}
	q.cleanStaleInstances()
	go q.worker()
	return q
}

func (q *LaunchQueue) track(uuid string, p *os.Process) {
	q.procMu.Lock()
	defer q.procMu.Unlock()
	if p == nil {
		if old := q.procs[uuid]; old != nil {
			unregisterLauncherPid(uint32(old.Pid))
		}
		delete(q.procs, uuid)
		return
	}
	q.procs[uuid] = p
	registerLauncherPid(uint32(p.Pid))
}

func (q *LaunchQueue) KillProc(uuid string) {
	q.procMu.Lock()
	p := q.procs[uuid]
	delete(q.procs, uuid)
	q.procMu.Unlock()
	if p != nil {
		unregisterLauncherPid(uint32(p.Pid))
		_ = p.Kill()
	}
}

func (q *LaunchQueue) Enqueue(uuid, client string) bool {
	q.mu.Lock()
	if q.pending[uuid] {
		q.mu.Unlock()
		return false
	}
	q.pending[uuid] = true
	q.client[uuid] = client
	q.order = append(q.order, uuid)
	q.mu.Unlock()
	q.tracker.noteLaunch(uuid)
	q.jobs <- uuid
	return true
}

func (q *LaunchQueue) worker() {
	for uuid := range q.jobs {
		q.mu.Lock()
		client := q.client[uuid]
		q.current = uuid
		q.mu.Unlock()
		q.run(uuid, client)
		q.mu.Lock()
		q.current = ""
		delete(q.pending, uuid)
		delete(q.client, uuid)
		q.order = removeFromOrder(q.order, uuid)
		q.mu.Unlock()
	}
}

func removeFromOrder(order []string, uuid string) []string {
	out := order[:0]
	for _, u := range order {
		if u != uuid {
			out = append(out, u)
		}
	}
	return out
}

// QueueEntry describes one account in the launch queue for the UI.
type QueueEntry struct {
	UUID  string `json:"uuid"`
	Name  string `json:"name"`
	State string `json:"state"` // "launching" (head, mid-launch) or "pending"
}

// QueueStatus returns the accounts still waiting to launch, in FIFO order.
// Accounts whose game window is already up have left the queue and are dropped.
func (q *LaunchQueue) QueueStatus() []QueueEntry {
	running, _ := q.tracker.Resolve()
	q.mu.Lock()
	order := append([]string(nil), q.order...)
	current := q.current
	q.mu.Unlock()
	out := []QueueEntry{}
	for _, uuid := range order {
		if running[uuid] != 0 {
			continue
		}
		state := "pending"
		if uuid == current {
			state = "launching"
		}
		name := ""
		if acc, ok := q.vault.Get(uuid); ok {
			name = acc.Name
		}
		out = append(out, QueueEntry{UUID: uuid, Name: name, State: state})
	}
	return out
}

func AccountLaunchOpts(acc *vault.Account) vault.LaunchOpts {
	ram := acc.Ram
	if (acc.Minimal || acc.Profile == MinimalProfileName) && ram < 1024 {
		ram = 1024
	}
	return vault.LaunchOpts{
		Ram:         ram,
		MinGraphics: acc.MinGraphics,
		Fullscreen:  acc.Fullscreen,
		DiscordRPC:  acc.DiscordRPC,
		AutoEnter:   acc.AutoEnter,
		DebugMode:   acc.DebugMode,
		Minimal:     acc.Minimal,
	}
}

func (q *LaunchQueue) launchExe(uuid, name, client, exe, url string, before []uint32) (bool, <-chan bool) {
	if err := EnsureLauncherFrom(exe, url); err != nil {
		if q.logs != nil {
			q.logs.unsupported(uuid, name, "[AccountChanger] Не удалось подготовить лаунчер: "+err.Error())
		}
		return false, nil
	}
	home := q.instanceHome(uuid, client)
	q.launchLog("'%s': exe-лаунчер, exe=%s, home=%s", name, exe, home)
	var launcherPID uint32
	err := StartLauncherLogged(exe, home, uuid, name, nil, func(p *os.Process) {
		q.track(uuid, p)
		if p != nil {
			launcherPID = uint32(p.Pid)
		}
	}, nil)
	if err != nil {
		q.launchLog("'%s': запуск exe-лаунчера НЕ УДАЛСЯ: %v", name, err)
		if q.logs != nil {
			q.logs.unsupported(uuid, name, "[AccountChanger] Не удалось запустить лаунчер: "+err.Error())
		}
		return false, nil
	}
	q.launchLog("'%s': exe-лаунчер запущен, bootstrap pid=%d", name, launcherPID)
	return true, q.tailGame(uuid, name, client, before, launcherPID)
}

func (q *LaunchQueue) tailGame(uuid, name, client string, before []uint32, launcherPID uint32) <-chan bool {
	ready := make(chan bool, 1)
	go TailGameLog(q.paths.Updates, client, uuid, name, q.logs, func(ok bool) {
		if ok {
			q.launchLog("'%s': игровой лог найден (клиент запускается)", name)
			go func() {
				gpid := q.tracker.bindVerifiedLauncher(uuid, launcherPID)
				if gpid == 0 {
					gpid = q.tracker.bindGame(uuid, before)
				}
				q.launchLog("'%s': игра привязана, game pid=%d (по launcherPID=%d)", name, gpid, launcherPID)
				q.wipeInstanceToken(uuid)
				q.finishLogWhenGameCloses(uuid)
				q.launchLog("'%s': игра закрыта, инстанс очищен", name)
				q.removeInstanceDir(uuid)
			}()
		} else {
			q.launchLog("'%s': игровой лог НЕ найден в отведённое время", name)
		}
		ready <- ok
	})
	return ready
}

func (q *LaunchQueue) linkShared(dst, src string) {
	LinkSharedDir(dst, src)
}

func (q *LaunchQueue) instanceHome(uuid, client string) string {
	home := filepath.Join(q.paths.Data, "instances", uuid)
	cris := filepath.Join(home, ".cristalix")
	if err := os.MkdirAll(cris, 0o755); err != nil {
		return ""
	}
	q.linkShared(filepath.Join(cris, "runtime"), filepath.Join(q.paths.Cristalix, "runtime"))
	cfg := filepath.Join(cris, ".launcher")
	if data, err := os.ReadFile(q.paths.LauncherCfg); err == nil {
		_ = os.WriteFile(cfg, data, 0o644)
	}
	if acc, ok := q.vault.Get(uuid); ok {
		effective := client
		if effective == "" {
			effective = acc.Client
		}
		if effective == "" {
			effective = CurrentClient(q.paths.LauncherCfg)
		}
		_ = ApplyAccount(cfg, acc.Name, acc.Token, effective, AccountLaunchOpts(acc))
		q.launchLog("instance '%s': токен записан в %s (client=%s, ram=%d, minimal=%v)", acc.Name, cfg, effective, AccountLaunchOpts(acc).Ram, acc.Minimal)
	}
	return home
}

func (q *LaunchQueue) launchLog(format string, args ...any) {
	line := fmt.Sprintf("["+time.Now().Format("15:04:05")+"] "+format+"\n", args...)
	f, err := os.OpenFile(filepath.Join(q.paths.Data, "launch.log"), os.O_CREATE|os.O_WRONLY|os.O_APPEND, 0o644)
	if err != nil {
		return
	}
	defer f.Close()
	_, _ = f.WriteString(line)
}

func dirExists(p string) bool {
	fi, err := os.Stat(p)
	return err == nil && fi.IsDir()
}

func (q *LaunchQueue) instanceDir(uuid string) string {
	return filepath.Join(q.paths.Data, "instances", uuid)
}

func (q *LaunchQueue) wipeInstanceToken(uuid string) {
	cfg := filepath.Join(q.instanceDir(uuid), ".cristalix", ".launcher")
	if _, err := os.Stat(cfg); err == nil {
		_ = AnnulAccount(cfg)
	}
}

func (q *LaunchQueue) removeInstanceDir(uuid string) {
	dir := q.instanceDir(uuid)
	_ = os.Remove(filepath.Join(dir, ".cristalix", "runtime"))
	_ = os.RemoveAll(dir)
}

func (q *LaunchQueue) cleanStaleInstances() {
	base := filepath.Join(q.paths.Data, "instances")
	entries, err := os.ReadDir(base)
	if err != nil {
		return
	}
	for _, e := range entries {
		if e.IsDir() {
			q.removeInstanceDir(e.Name())
		}
	}
}

func (q *LaunchQueue) launchJar(uuid, name, client string, before []uint32) (bool, <-chan bool) {
	home := q.instanceHome(uuid, client)
	java := ResolveJava(q.paths.Cristalix)
	q.launchLog("'%s': jar-лаунчер, java=%s, jar=%s, home=%s", name, java, q.paths.LauncherJar, home)
	var launcherPID uint32
	err := StartLauncherJar(java, q.paths.LauncherJar, home, func() {
		q.launchLog("'%s': jar битый/не запустился, фолбэк на staff exe", name)
		if EnsureLauncherFrom(q.paths.StaffLauncherExe, StaffLauncherURL) != nil {
			return
		}
		_ = StartLauncherLogged(q.paths.StaffLauncherExe, home, uuid, name, nil, func(p *os.Process) {
			q.track(uuid, p)
		}, nil)
	}, func(p *os.Process) {
		q.track(uuid, p)
		if p != nil {
			launcherPID = uint32(p.Pid)
		}
	})
	if err != nil {
		q.launchLog("'%s': запуск jar-лаунчера НЕ УДАЛСЯ: %v", name, err)
		if q.logs != nil {
			q.logs.unsupported(uuid, name, "[AccountChanger] Не удалось запустить лаунчер: "+err.Error())
		}
		return false, nil
	}
	q.launchLog("'%s': jar-лаунчер запущен, bootstrap pid=%d", name, launcherPID)
	return true, q.tailGame(uuid, name, client, before, launcherPID)
}

func (q *LaunchQueue) finishLogWhenGameCloses(uuid string) {
	seen := false
	misses := 0
	giveUp := time.Now().Add(gameAppearWindow)
	for {
		running, _ := q.tracker.Resolve()
		if running[uuid] != 0 {
			seen = true
			misses = 0
		} else if seen {
			misses++
			if misses >= 2 {
				q.logs.finish(uuid)
				return
			}
		} else if time.Now().After(giveUp) {
			return
		}
		time.Sleep(1 * time.Second)
	}
}

func (q *LaunchQueue) launchFor(uuid, name, client string, before []uint32) (bool, bool, <-chan bool) {
	switch q.cfg.Launcher() {
	case config.LauncherCustom:
		if q.logs != nil {
			q.logs.unsupported(uuid, name, "Этот лаунчер не поддерживает чтение логов")
		}
		return StartLauncher(q.cfg.CustomLauncher()) == nil, false, nil
	case config.LauncherNormal:
		ok, ready := q.launchExe(uuid, name, client, q.paths.LauncherExe, launcherDownloadURL, before)
		return ok, true, ready
	case config.LauncherNew:
		ok, ready := q.launchExe(uuid, name, client, q.paths.StaffLauncherExe, StaffLauncherURL, before)
		return ok, true, ready
	default:
		if EnsureLauncherFrom(q.paths.LauncherJar, JarLauncherURL) != nil {
			ok, ready := q.launchExe(uuid, name, client, q.paths.StaffLauncherExe, StaffLauncherURL, before)
			return ok, true, ready
		}
		ok, ready := q.launchJar(uuid, name, client, before)
		return ok, true, ready
	}
}

func (q *LaunchQueue) run(uuid, client string) {
	acc, ok := q.vault.Get(uuid)
	if !ok || acc.Name == "" || acc.Token == "" {
		q.launchLog("run %s: пропуск — нет аккаунта или токена (ok=%v)", shortUUID(uuid), ok)
		return
	}
	q.launchLog("=== запуск '%s' (client=%s, launcher=%s, aggressive=%v, autoPlay=%v) ===", acc.Name, client, q.cfg.Launcher(), q.cfg.AggressiveLaunch(), q.cfg.AutoPlay())
	if running, _ := q.tracker.Resolve(); running[uuid] != 0 {
		q.launchLog("'%s': уже запущен, пропуск", acc.Name)
		return
	}
	if client != "" {
		q.vault.SetClient(uuid, client)
	}
	effective := client
	if effective == "" {
		effective = acc.Client
	}
	if effective == "" {
		effective = CurrentClient(q.paths.LauncherCfg)
	}
	platform.ClearStaleLocks(q.paths.Cristalix)
	if q.cfg.Launcher() == config.LauncherCustom {
		if ApplyAccount(q.paths.LauncherCfg, acc.Name, acc.Token, effective, AccountLaunchOpts(acc)) != nil {
			return
		}
	}
	if effective != "" {
		updates := UpdatesDir(q.paths.LauncherCfg, q.paths.Updates)
		clientDir := filepath.Join(updates, effective)
		q.launchLog("'%s': настройки — updates=%s, clientDir=%s (существует=%v)", acc.Name, updates, clientDir, dirExists(clientDir))
		if acc.Profile != "" && acc.Profile != MinimalProfileName {
			applyProfile(q.paths.Profiles, acc.Profile, clientDir)
			q.launchLog("'%s': профиль '%s' скопирован в %s", acc.Name, acc.Profile, clientDir)
			if launchIsStaff(acc.Name) {
				applyProfile(q.paths.Profiles, acc.Profile, filepath.Join(updates, stagingClient))
			}
		}
		if acc.Minimal || acc.Profile == MinimalProfileName {
			applyMinimalOptionsAll(updates)
			q.launchLog("'%s': минимальные настройки применены к клиентам: %v", acc.Name, ListClients(updates))
		} else {
			applyClientOptionsAll(updates, acc)
			q.launchLog("'%s': клиентские настройки (chunks=%d,fps=%d,anim=%d,fastRender=%d) применены к: %v", acc.Name, acc.RenderDistance, acc.MaxFps, acc.Animations, acc.FastRender, ListClients(updates))
		}
	} else {
		q.launchLog("'%s': effective client ПУСТ — настройки НЕ применяются", acc.Name)
	}
	before := gameWindowPids()
	launched, _, _ := q.launchFor(uuid, acc.Name, effective, before)
	if !launched {
		q.launchLog("'%s': лаунчер НЕ запустился", acc.Name)
		return
	}
	q.vault.MarkLaunched(uuid)
	if q.cfg.AutoPlay() {
		q.launchLog("'%s': жму ИГРАТЬ (автозапуск)", acc.Name)
		go q.autoPlay(uuid)
	}
	q.procMu.Lock()
	var launcherPID uint32
	if lp := q.procs[uuid]; lp != nil {
		launcherPID = uint32(lp.Pid)
		before = append(before, launcherPID)
	}
	q.procMu.Unlock()
	beforeCopy := append([]uint32(nil), before...)
	bind := func() {
		if launcherPID == 0 || q.tracker.bindVerifiedLauncher(uuid, launcherPID) == 0 {
			q.tracker.bindGame(uuid, beforeCopy)
		}
	}
	if q.cfg.AggressiveLaunch() {
		go bind()
	} else {
		q.launchLog("'%s': обычный режим — ждём поднятия игры перед следующим", acc.Name)
		bind()
		q.launchLog("'%s': игра поднялась, можно запускать следующий", acc.Name)
	}
}

func (q *LaunchQueue) autoPlay(uuid string) {
	clickPlayButton(autoPlayTimeout)
}

func (q *LaunchQueue) PauseGroup() bool {
	q.mu.Lock()
	defer q.mu.Unlock()
	if !q.groupActive || q.groupPaused {
		return false
	}
	q.groupPaused = true
	q.groupResumeCh = make(chan struct{})
	return true
}

func (q *LaunchQueue) ResumeGroup() bool {
	q.mu.Lock()
	defer q.mu.Unlock()
	if !q.groupActive || !q.groupPaused {
		return false
	}
	q.groupPaused = false
	if q.groupResumeCh != nil {
		close(q.groupResumeCh)
		q.groupResumeCh = nil
	}
	return true
}

func (q *LaunchQueue) CancelGroup() bool {
	q.mu.Lock()
	defer q.mu.Unlock()
	if !q.groupActive {
		return false
	}
	q.groupCancelled = true
	if q.groupPaused {
		q.groupPaused = false
		if q.groupResumeCh != nil {
			close(q.groupResumeCh)
			q.groupResumeCh = nil
		}
	}
	return true
}

type GroupSkip struct {
	Name   string `json:"name"`
	Reason string `json:"reason"`
}

func (q *LaunchQueue) GroupProgress() (active, paused bool, done, total int, skips []GroupSkip) {
	q.mu.Lock()
	defer q.mu.Unlock()
	return q.groupActive, q.groupPaused, q.groupDone, q.groupTotal, append([]GroupSkip(nil), q.groupSkips...)
}

func (q *LaunchQueue) recordGroupSkip(name, reason string) {
	q.mu.Lock()
	q.groupSkips = append(q.groupSkips, GroupSkip{Name: name, Reason: reason})
	q.mu.Unlock()
}

func (q *LaunchQueue) waitIfGroupPaused() {
	for {
		q.mu.Lock()
		if !q.groupPaused || q.groupCancelled {
			q.mu.Unlock()
			return
		}
		ch := q.groupResumeCh
		q.mu.Unlock()
		<-ch
	}
}

func (q *LaunchQueue) LaunchGroup(members []string, groupProfile string) {
	q.mu.Lock()
	if q.groupActive {
		q.mu.Unlock()
		return
	}
	q.launchLog("=== ГРУППА: запуск %d аккаунтов (profile=%q, aggressive=%v) ===", len(members), groupProfile, q.cfg.AggressiveLaunch())
	q.groupActive = true
	q.groupPaused = false
	q.groupCancelled = false
	q.groupDone = 0
	q.groupTotal = len(members)
	q.groupSkips = nil
	q.mu.Unlock()
	defer func() {
		q.mu.Lock()
		q.groupActive = false
		q.groupPaused = false
		q.groupCancelled = false
		q.groupDone = 0
		q.groupTotal = 0
		q.mu.Unlock()
	}()

	applied := map[string]bool{}
	autoPlayStarted := false
	groupHasStaff := false
	for _, uuid := range members {
		if a, ok := q.vault.Get(uuid); ok && a.Name != "" && launchIsStaff(a.Name) {
			groupHasStaff = true
			break
		}
	}
	for _, uuid := range members {
		q.waitIfGroupPaused()
		q.mu.Lock()
		cancelled := q.groupCancelled
		q.mu.Unlock()
		if cancelled {
			break
		}
		func() {
			defer func() {
				q.mu.Lock()
				q.groupDone++
				q.mu.Unlock()
			}()
			acc, ok := q.vault.Get(uuid)
			if !ok || acc.Name == "" || acc.Token == "" {
				q.recordGroupSkip(shortUUID(uuid), "нет сохранённого токена")
				return
			}
			if running, _ := q.tracker.Resolve(); running[uuid] != 0 {
				q.recordGroupSkip(acc.Name, "уже запущен")
				return
			}
			effective := acc.Client
			if effective == "" {
				effective = CurrentClient(q.paths.LauncherCfg)
			}
			platform.ClearStaleLocks(q.paths.Cristalix)
			if q.cfg.Launcher() == config.LauncherCustom {
				if ApplyAccount(q.paths.LauncherCfg, acc.Name, acc.Token, effective, AccountLaunchOpts(acc)) != nil {
					q.recordGroupSkip(acc.Name, "не удалось применить токен")
					return
				}
			}
			profile := groupProfile
			if profile == "" {
				profile = acc.Profile
			}
			updates := UpdatesDir(q.paths.LauncherCfg, q.paths.Updates)
			if effective != "" {
				clientDir := filepath.Join(updates, effective)
				if profile != "" && profile != MinimalProfileName && !applied[clientDir] {
					applyProfile(q.paths.Profiles, profile, clientDir)
					applied[clientDir] = true
					if groupHasStaff {
						applyProfile(q.paths.Profiles, profile, filepath.Join(updates, stagingClient))
					}
				}
			}
			if acc.Minimal || profile == MinimalProfileName {
				applyMinimalOptionsAll(updates)
			} else {
				applyClientOptionsAll(updates, acc)
			}
			q.tracker.noteLaunch(uuid)
			q.launchLog("группа: запускаю '%s' (client=%s)", acc.Name, effective)
			before := gameWindowPids()
			launched, _, _ := q.launchFor(uuid, acc.Name, effective, before)
			if !launched {
				q.launchLog("группа: '%s' — лаунчер не запустился", acc.Name)
				q.recordGroupSkip(acc.Name, "лаунчер не запустился")
				return
			}
			q.vault.MarkLaunched(uuid)
			if q.cfg.AutoPlay() && !autoPlayStarted {
				autoPlayStarted = true
				go q.autoPlay(uuid)
			}
			q.procMu.Lock()
			var launcherPID uint32
			if lp := q.procs[uuid]; lp != nil {
				launcherPID = uint32(lp.Pid)
				before = append(before, launcherPID)
			}
			q.procMu.Unlock()
			beforeCopy := append([]uint32(nil), before...)
			bind := func() {
				if launcherPID == 0 || q.tracker.bindVerifiedLauncher(uuid, launcherPID) == 0 {
					q.tracker.bindGame(uuid, beforeCopy)
				}
			}
			if q.cfg.AggressiveLaunch() {
				go bind()
			} else {
				bind()
			}
		}()
	}
}
