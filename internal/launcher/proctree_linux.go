package launcher

import (
	"os"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
)

type procInfo struct {
	pid     uint32
	ppid    uint32
	comm    string
	cmdline string
}

func gameWindowPids() []uint32 {
	var pids []uint32
	for _, p := range scanProcs(true) {
		if !p.isJava() || !p.isGame() {
			continue
		}
		pids = append(pids, p.pid)
	}
	sort.Slice(pids, func(i, j int) bool { return pids[i] < pids[j] })
	return pids
}

func (p procInfo) isJava() bool {
	if strings.Contains(strings.ToLower(p.comm), "java") {
		return true
	}
	first, _, _ := strings.Cut(p.cmdline, " ")
	return strings.Contains(strings.ToLower(filepath.Base(first)), "java")
}

func (p procInfo) isGame() bool {
	if strings.Contains(p.cmdline, "Cristalix.jar") || strings.Contains(p.cmdline, "CristalixLauncher") {
		return false
	}
	return strings.Contains(p.cmdline, "minecraft.jar") || strings.Contains(p.cmdline, "net.minecraft.client")
}

func javaProcessPids() map[uint32]bool {
	out := map[uint32]bool{}
	for _, p := range scanProcs(true) {
		if p.isJava() {
			out[p.pid] = true
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
	parents := map[uint32]uint32{}
	for _, p := range scanProcs(false) {
		parents[p.pid] = p.ppid
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

func scanProcs(withCmdline bool) []procInfo {
	dir, err := os.Open("/proc")
	if err != nil {
		return nil
	}
	defer dir.Close()
	names, err := dir.Readdirnames(-1)
	if err != nil {
		return nil
	}

	out := make([]procInfo, 0, len(names))
	for _, name := range names {
		pid, err := strconv.ParseUint(name, 10, 32)
		if err != nil {
			continue
		}
		info, ok := readProc(uint32(pid), withCmdline)
		if !ok {
			continue
		}
		out = append(out, info)
	}
	return out
}

func readProc(pid uint32, withCmdline bool) (procInfo, bool) {
	raw, err := os.ReadFile("/proc/" + strconv.FormatUint(uint64(pid), 10) + "/stat")
	if err != nil {
		return procInfo{}, false
	}
	info := procInfo{pid: pid}

	s := string(raw)
	openIdx := strings.IndexByte(s, '(')
	closeIdx := strings.LastIndexByte(s, ')')
	if openIdx < 0 || closeIdx < 0 || closeIdx < openIdx {
		return procInfo{}, false
	}
	info.comm = s[openIdx+1 : closeIdx]

	rest := strings.Fields(s[closeIdx+1:])
	if len(rest) < 2 {
		return procInfo{}, false
	}
	ppid, err := strconv.ParseUint(rest[1], 10, 32)
	if err != nil {
		return procInfo{}, false
	}
	info.ppid = uint32(ppid)

	if withCmdline {
		if cmd, err := os.ReadFile("/proc/" + strconv.FormatUint(uint64(pid), 10) + "/cmdline"); err == nil {
			info.cmdline = strings.TrimSpace(strings.ReplaceAll(string(cmd), "\x00", " "))
		}
	}
	return info, true
}
