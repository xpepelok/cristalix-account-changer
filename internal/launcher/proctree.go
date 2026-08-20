package launcher

import (
	"sort"
	"strings"
	"sync"
)

var titleAllowMu sync.Mutex
var titleAllow = map[string]bool{}

func AllowGameTitles(names []string) {
	for _, n := range names {
		allowGameTitle(n)
	}
}

func allowGameTitle(name string) {
	low := strings.ToLower(strings.TrimSpace(name))
	if low == "" {
		return
	}
	titleAllowMu.Lock()
	titleAllow[low] = true
	titleAllowMu.Unlock()
}

func allowedTitles() []string {
	titleAllowMu.Lock()
	defer titleAllowMu.Unlock()
	out := make([]string, 0, len(titleAllow)+1)
	out = append(out, defaultGameTitle)
	for name := range titleAllow {
		out = append(out, name)
	}
	sort.Strings(out[1:])
	return out
}

func isGameTitle(title string) bool {
	low := strings.ToLower(strings.TrimSpace(title))
	if low == "cristalix" || strings.HasPrefix(low, "cristalix ") {
		return true
	}
	titleAllowMu.Lock()
	defer titleAllowMu.Unlock()
	return titleAllow[low]
}

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
