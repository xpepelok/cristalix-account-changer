package launcher

import (
	"accountchanger/internal/config"
	"accountchanger/internal/platform"
	"accountchanger/internal/player"
	"accountchanger/internal/vault"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"
)

const launchingWindow = 2 * time.Minute
const gameAppearWindow = 15 * time.Minute
const tokenConsumeDelay = 6 * time.Second
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
	go q.worker()
	return q
}

func (q *LaunchQueue) track(uuid string, p *os.Process) {
	q.procMu.Lock()
	defer q.procMu.Unlock()
	if p == nil {
		delete(q.procs, uuid)
		return
	}
	q.procs[uuid] = p
}

func (q *LaunchQueue) KillProc(uuid string) {
	q.procMu.Lock()
	p := q.procs[uuid]
	delete(q.procs, uuid)
	q.procMu.Unlock()
	if p != nil {
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
	q.mu.Unlock()
	q.tracker.noteLaunch(uuid)
	q.jobs <- uuid
	return true
}

func (q *LaunchQueue) worker() {
	for uuid := range q.jobs {
		q.mu.Lock()
		client := q.client[uuid]
		q.mu.Unlock()
		q.run(uuid, client)
		q.mu.Lock()
		delete(q.pending, uuid)
		delete(q.client, uuid)
		q.mu.Unlock()
	}
}

func AccountLaunchOpts(acc *vault.Account) vault.LaunchOpts {
	return vault.LaunchOpts{
		Ram:         acc.Ram,
		MinGraphics: acc.MinGraphics,
		Fullscreen:  acc.Fullscreen,
		DiscordRPC:  acc.DiscordRPC,
		AutoEnter:   acc.AutoEnter,
		DebugMode:   acc.DebugMode,
	}
}

func (q *LaunchQueue) launchExe(uuid, name, client, exe, url string, before []uint32) (bool, <-chan bool) {
	if err := EnsureLauncherFrom(exe, url); err != nil {
		if q.logs != nil {
			q.logs.unsupported(uuid, name, "[AccountChanger] Не удалось подготовить лаунчер: "+err.Error())
		}
		return false, nil
	}
	var launcherPID uint32
	err := StartLauncherLogged(exe, uuid, name, nil, func(p *os.Process) {
		q.track(uuid, p)
		if p != nil {
			launcherPID = uint32(p.Pid)
		}
	}, nil)
	if err != nil {
		if q.logs != nil {
			q.logs.unsupported(uuid, name, "[AccountChanger] Не удалось запустить лаунчер: "+err.Error())
		}
		return false, nil
	}
	return true, q.tailGame(uuid, name, client, before, launcherPID)
}

func (q *LaunchQueue) tailGame(uuid, name, client string, before []uint32, launcherPID uint32) <-chan bool {
	ready := make(chan bool, 1)
	go TailGameLog(q.paths.Updates, client, uuid, name, q.logs, func(ok bool) {
		if ok {
			go func() {
				if q.tracker.bindVerifiedLauncher(uuid, launcherPID) == 0 {
					q.tracker.bindGame(uuid, before)
				}
				q.finishLogWhenGameCloses(uuid)
			}()
		}
		ready <- ok
	})
	return ready
}

func (q *LaunchQueue) launchJar(uuid, name, client string, before []uint32) (bool, <-chan bool) {
	java := ResolveJava(q.paths.Cristalix)
	var launcherPID uint32
	err := StartLauncherJar(java, q.paths.LauncherJar, func() {
		if EnsureLauncherFrom(q.paths.StaffLauncherExe, StaffLauncherURL) != nil {
			return
		}
		_ = StartLauncherLogged(q.paths.StaffLauncherExe, uuid, name, nil, func(p *os.Process) {
			q.track(uuid, p)
		}, nil)
	}, func(p *os.Process) {
		q.track(uuid, p)
		if p != nil {
			launcherPID = uint32(p.Pid)
		}
	})
	if err != nil {
		if q.logs != nil {
			q.logs.unsupported(uuid, name, "[AccountChanger] Не удалось запустить лаунчер: "+err.Error())
		}
		return false, nil
	}
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

func waitLogReady(ch <-chan bool) bool {
	if ch == nil {
		return true
	}
	select {
	case ok := <-ch:
		return ok
	case <-time.After(50 * time.Second):
		return false
	}
}

func (q *LaunchQueue) run(uuid, client string) {
	acc, ok := q.vault.Get(uuid)
	if !ok || acc.Name == "" || acc.Token == "" {
		return
	}
	if running, _ := q.tracker.Resolve(); running[uuid] != 0 {
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
	if ApplyAccount(q.paths.LauncherCfg, acc.Name, acc.Token, effective, AccountLaunchOpts(acc)) != nil {
		return
	}
	if effective != "" {
		updates := UpdatesDir(q.paths.LauncherCfg, q.paths.Updates)
		clientDir := filepath.Join(updates, effective)
		if acc.Profile != "" {
			applyProfile(q.paths.Profiles, acc.Profile, clientDir)
			if launchIsStaff(acc.Name) {
				applyProfile(q.paths.Profiles, acc.Profile, filepath.Join(updates, stagingClient))
			}
		}
		applyClientOptionsAll(updates, acc)
	}
	before := gameWindowPids()
	launched, nativeTracked, logReady := q.launchFor(uuid, acc.Name, effective, before)
	if !launched {
		return
	}
	q.vault.MarkLaunched(uuid)
	if q.cfg.AutoPlay() {
		q.autoPlay(uuid)
	}
	if nativeTracked && !waitLogReady(logReady) {
		return
	}
	q.procMu.Lock()
	if lp := q.procs[uuid]; lp != nil {
		before = append(before, uint32(lp.Pid))
	}
	q.procMu.Unlock()
	if !nativeTracked {
		q.tracker.bindGame(uuid, before)
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
			if ApplyAccount(q.paths.LauncherCfg, acc.Name, acc.Token, effective, AccountLaunchOpts(acc)) != nil {
				q.recordGroupSkip(acc.Name, "не удалось применить токен")
				return
			}
			profile := groupProfile
			if profile == "" {
				profile = acc.Profile
			}
			updates := UpdatesDir(q.paths.LauncherCfg, q.paths.Updates)
			if effective != "" {
				clientDir := filepath.Join(updates, effective)
				if profile != "" && !applied[clientDir] {
					applyProfile(q.paths.Profiles, profile, clientDir)
					applied[clientDir] = true
					if groupHasStaff {
						applyProfile(q.paths.Profiles, profile, filepath.Join(updates, stagingClient))
					}
				}
			}
			applyClientOptionsAll(updates, acc)
			q.tracker.noteLaunch(uuid)
			before := gameWindowPids()
			launched, nativeTracked, logReady := q.launchFor(uuid, acc.Name, effective, before)
			if !launched {
				q.recordGroupSkip(acc.Name, "лаунчер не запустился")
				return
			}
			q.vault.MarkLaunched(uuid)
			if q.cfg.AutoPlay() {
				q.autoPlay(uuid)
			}
			if nativeTracked && !waitLogReady(logReady) {
				q.recordGroupSkip(acc.Name, "лаунчер не подтвердил вход за 50с")
				time.Sleep(tokenConsumeDelay)
				return
			}
			q.procMu.Lock()
			if lp := q.procs[uuid]; lp != nil {
				before = append(before, uint32(lp.Pid))
			}
			q.procMu.Unlock()
			if !nativeTracked {
				q.tracker.bindGame(uuid, before)
			}
			time.Sleep(tokenConsumeDelay)
		}()
	}
}
