package launcher

import (
	"accountchanger/internal/config"
	"accountchanger/internal/platform"
	"accountchanger/internal/vault"
	"os"
	"path/filepath"
	"sync"
	"time"
)

const launchingWindow = 2 * time.Minute
const tokenConsumeDelay = 6 * time.Second

type LaunchQueue struct {
	paths       platform.Paths
	vault       *vault.Vault
	tracker     *GameTracker
	jobs        chan string
	mu          sync.Mutex
	pending     map[string]bool
	client      map[string]string
	groupActive bool
	logs        *LogStore
	cfg         *config.ConfigStore
	procMu      sync.Mutex
	procs       map[string]*os.Process
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

func (q *LaunchQueue) launchExe(uuid, name, exe, url string) bool {
	if EnsureLauncherFrom(exe, url) != nil {
		return false
	}
	if q.logs != nil {
		q.logs.unsupported(uuid, name, "Этот лаунчер не поддерживает чтение логов")
	}
	return StartLauncher(exe) == nil
}

func (q *LaunchQueue) launchFor(uuid, name string) bool {
	switch q.cfg.Launcher() {
	case config.LauncherNormal:
		return q.launchExe(uuid, name, q.paths.LauncherExe, launcherDownloadURL)
	case config.LauncherNew:
		return q.launchExe(uuid, name, q.paths.StaffLauncherExe, StaffLauncherURL)
	default:
		if EnsureLauncherFrom(q.paths.LauncherJar, JarLauncherURL) != nil {
			return q.launchExe(uuid, name, q.paths.StaffLauncherExe, StaffLauncherURL)
		}
		java := ResolveJava(q.paths.Cristalix)
		return StartLauncherJar(java, q.paths.LauncherJar, uuid, name, q.logs, func() {
			q.launchExe(uuid, name, q.paths.StaffLauncherExe, StaffLauncherURL)
		}, func(p *os.Process) {
			q.track(uuid, p)
		}) == nil
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
		}
		applyClientOptionsAll(updates, acc)
	}
	if !q.launchFor(uuid, acc.Name) {
		return
	}
	q.vault.MarkLaunched(uuid)
	if q.cfg.AutoPlay() {
		q.autoPlay(uuid)
	}
	time.Sleep(3 * time.Second)
}

func (q *LaunchQueue) autoPlay(uuid string) {
	q.procMu.Lock()
	p := q.procs[uuid]
	q.procMu.Unlock()
	if p != nil {
		ClickPlayButtonForPid(p.Pid, autoPlayTimeout)
		return
	}
	clickPlayButton(autoPlayTimeout)
}

func (q *LaunchQueue) LaunchGroup(members []string, groupProfile string) {
	q.mu.Lock()
	if q.groupActive {
		q.mu.Unlock()
		return
	}
	q.groupActive = true
	q.mu.Unlock()
	defer func() {
		q.mu.Lock()
		q.groupActive = false
		q.mu.Unlock()
	}()

	applied := map[string]bool{}
	baseline := len(gameWindowPids())
	launched := 0
	for _, uuid := range members {
		acc, ok := q.vault.Get(uuid)
		if !ok || acc.Name == "" || acc.Token == "" {
			continue
		}
		if running, _ := q.tracker.Resolve(); running[uuid] != 0 {
			continue
		}
		effective := acc.Client
		if effective == "" {
			effective = CurrentClient(q.paths.LauncherCfg)
		}
		platform.ClearStaleLocks(q.paths.Cristalix)
		if ApplyAccount(q.paths.LauncherCfg, acc.Name, acc.Token, effective, AccountLaunchOpts(acc)) != nil {
			continue
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
			}
		}
		applyClientOptionsAll(updates, acc)
		q.tracker.noteLaunch(uuid)
		if !q.launchFor(uuid, acc.Name) {
			continue
		}
		q.vault.MarkLaunched(uuid)
		if q.cfg.AutoPlay() {
			q.autoPlay(uuid)
		}
		launched++
		waitForWindowCount(baseline+launched, 120*time.Second)
		time.Sleep(tokenConsumeDelay)
	}
}

func waitForWindowCount(target int, timeout time.Duration) {
	deadline := time.Now().Add(timeout)
	for time.Now().Before(deadline) {
		if len(gameWindowPids()) >= target {
			return
		}
		time.Sleep(1 * time.Second)
	}
}
