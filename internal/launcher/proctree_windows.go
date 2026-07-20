package launcher

import (
	"accountchanger/internal/platform"
	"sort"
	"strings"
	"unsafe"

	"golang.org/x/sys/windows"
)

func gameWindowPids() []uint32 {
	titles := platform.WindowTitlesByPID()
	javaPids := javaProcessPids()

	var pids []uint32
	for pid := range titles {
		if !javaPids[pid] {
			continue
		}
		if isLauncherPid(pid) {
			continue
		}
		for _, title := range titles[pid] {
			low := strings.ToLower(strings.TrimSpace(title))
			if low == "cristalix" || strings.HasPrefix(low, "cristalix ") {
				pids = append(pids, pid)
				break
			}
		}
	}
	sort.Slice(pids, func(i, j int) bool { return pids[i] < pids[j] })
	return pids
}

func javaProcessPids() map[uint32]bool {
	out := map[uint32]bool{}
	snapshot, err := windows.CreateToolhelp32Snapshot(windows.TH32CS_SNAPPROCESS, 0)
	if err != nil {
		return out
	}
	defer windows.CloseHandle(snapshot)
	var entry windows.ProcessEntry32
	entry.Size = uint32(unsafe.Sizeof(entry))
	if windows.Process32First(snapshot, &entry) != nil {
		return out
	}
	for {
		name := strings.ToLower(windows.UTF16ToString(entry.ExeFile[:]))
		if strings.Contains(name, "java") {
			out[entry.ProcessID] = true
		}
		if windows.Process32Next(snapshot, &entry) != nil {
			break
		}
	}
	return out
}

func ProcessTreePids(root uint32) []uint32 {
	out := []uint32{root}
	for pid := range processDescendantsOf(root) {
		if pid != root {
			out = append(out, pid)
		}
	}
	return out
}

func javaDescendantsOf(root uint32) map[uint32]bool {
	out := map[uint32]bool{}
	java := javaProcessPids()
	for pid := range processDescendantsOf(root) {
		if java[pid] {
			out[pid] = true
		}
	}
	return out
}

func processDescendantsOf(root uint32) map[uint32]bool {
	out := map[uint32]bool{}
	if root == 0 {
		return out
	}
	snapshot, err := windows.CreateToolhelp32Snapshot(windows.TH32CS_SNAPPROCESS, 0)
	if err != nil {
		return out
	}
	defer windows.CloseHandle(snapshot)
	parents := map[uint32]uint32{}
	var entry windows.ProcessEntry32
	entry.Size = uint32(unsafe.Sizeof(entry))
	if windows.Process32First(snapshot, &entry) != nil {
		return out
	}
	for {
		parents[entry.ProcessID] = entry.ParentProcessID
		if windows.Process32Next(snapshot, &entry) != nil {
			break
		}
	}
	for pid := range parents {
		seen := map[uint32]bool{}
		for cur := pid; cur != 0 && !seen[cur]; cur = parents[cur] {
			if cur == root {
				out[pid] = true
				break
			}
			seen[cur] = true
		}
	}
	return out
}
