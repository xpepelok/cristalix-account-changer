package launcher

import "sync"

var launcherPidMu sync.Mutex
var launcherPids = map[uint32]bool{}

func registerLauncherPid(pid uint32) {
	if pid == 0 {
		return
	}
	launcherPidMu.Lock()
	launcherPids[pid] = true
	launcherPidMu.Unlock()
}

func unregisterLauncherPid(pid uint32) {
	if pid == 0 {
		return
	}
	launcherPidMu.Lock()
	delete(launcherPids, pid)
	launcherPidMu.Unlock()
}

func isLauncherPid(pid uint32) bool {
	launcherPidMu.Lock()
	defer launcherPidMu.Unlock()
	return launcherPids[pid]
}
