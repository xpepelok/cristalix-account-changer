package main

import (
	"accountchanger/internal/config"
	"accountchanger/internal/launcher"
	"accountchanger/internal/platform"
	"accountchanger/internal/stats"
	"accountchanger/internal/update"
	"accountchanger/internal/vault"
	"net"
	"net/http"
	"os"
	"runtime"
	"runtime/debug"
	"time"
)

const listenAddr = "127.0.0.1:47821"
const appURL = "http://127.0.0.1:47821/"

func main() {
	runtime.LockOSThread()
	debug.SetGCPercent(25)
	go func() {
		time.Sleep(8 * time.Second)
		platform.TrimMemory()
		for {
			time.Sleep(60 * time.Second)
			platform.TrimMemory()
		}
	}()
	update.CleanupOldExe()

	paths := platform.Resolve()

	updated := false
	for _, a := range os.Args[1:] {
		if a == "--updated" {
			updated = true
		}
	}

	attempts := 1
	if updated {
		attempts = 20
	}
	var listener net.Listener
	var err error
	for i := 0; i < attempts; i++ {
		listener, err = net.Listen("tcp", listenAddr)
		if err == nil {
			break
		}
		time.Sleep(300 * time.Millisecond)
	}
	if err != nil {
		_, _ = http.Post(appURL+"api/focus", "application/json", nil)
		return
	}
	_ = paths.Ensure()
	launcher.ClearWebCache(paths.WebProfile)

	vaultStore := vault.OpenVault(paths.Vault)
	cfg := config.OpenConfig(paths.Config)
	if !launcherAllowed(cfg.Launcher()) {
		cfg.SetLauncher(config.LauncherJar)
	}
	go stats.Loop(paths, cfg)
	watcher := &launcher.Watcher{Paths: paths, Vault: vaultStore}
	tracker := launcher.NewGameTracker(paths.Session)
	launcher.SeedTracker(tracker, vaultStore)
	logs := launcher.NewLogStore(paths.Logs)

	go func() {
		_ = launcher.EnsureLauncherFrom(paths.LauncherJar, launcher.JarLauncherURL)
	}()
	go watcher.Run()

	srv := &Server{
		paths:   paths,
		vault:   vaultStore,
		watcher: watcher,
		tracker: tracker,
		queue:   launcher.NewLaunchQueue(paths, vaultStore, tracker, logs, cfg),
		logs:    logs,
		cfg:     cfg,
		quit:    make(chan struct{}),
	}

	httpServer := &http.Server{Handler: srv.handler()}
	go func() {
		_ = httpServer.Serve(listener)
	}()

	platform.InstallDesktopEntry(iconBytes)
	ran := platform.RunNativeWindow(appURL, paths.WebProfile, iconBytes, func(focus, quit func()) {
		srv.setFocus(focus)
		srv.restart = quit
	})
	if !ran {
		srv.restart = func() { close(srv.quit) }
		platform.OpenBrowser(appURL, paths.WebProfile)
		<-srv.quit
	}
	logs.Flush()
}
