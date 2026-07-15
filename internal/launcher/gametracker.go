package launcher

import (
	"accountchanger/internal/platform"
	"accountchanger/internal/vault"
	"encoding/json"
	"os"
	"sort"
	"strings"
	"sync"
	"time"
	"unsafe"

	"golang.org/x/sys/windows"
)

type launchRec struct {
	UUID   string    `json:"uuid"`
	At     time.Time `json:"at"`
	Pid    uint32    `json:"pid"`
	Before []uint32  `json:"before,omitempty"`
}

type GameTracker struct {
	mu       sync.Mutex
	launched []launchRec
	path     string
}

func NewGameTracker(path string) *GameTracker {
	t := &GameTracker{path: path}
	t.load()
	return t
}

func (t *GameTracker) load() {
	data, err := os.ReadFile(t.path)
	if err != nil {
		return
	}
	var recs []launchRec
	if json.Unmarshal(data, &recs) != nil {
		return
	}
	t.launched = recs
}

func (t *GameTracker) persist() {
	if t.path == "" {
		return
	}
	data, err := json.Marshal(t.launched)
	if err != nil {
		return
	}
	os.WriteFile(t.path, data, 0o644)
}

func SeedTracker(t *GameTracker, vault *vault.Vault) {
	t.mu.Lock()
	defer t.mu.Unlock()

	claimed := map[uint32]bool{}
	for _, r := range t.launched {
		if r.Pid != 0 {
			claimed[r.Pid] = true
		}
	}

	var free []uint32
	for _, p := range gameWindowPids() {
		if !claimed[p] {
			free = append(free, p)
		}
	}
	if len(free) == 0 {
		return
	}

	tracked := map[string]bool{}
	for _, r := range t.launched {
		tracked[r.UUID] = true
	}

	accounts := vault.List()
	sort.Slice(accounts, func(i, j int) bool { return accounts[i].LastLaunched > accounts[j].LastLaunched })
	idx := 0
	for _, acc := range accounts {
		if idx >= len(free) {
			break
		}
		if acc.LastLaunched == 0 || tracked[acc.UUID] {
			continue
		}
		at := time.Unix(acc.LastLaunched, 0)
		t.launched = append(t.launched, launchRec{UUID: acc.UUID, At: at, Pid: free[idx]})
		idx++
	}
	t.persist()
}

func (t *GameTracker) noteLaunch(uuid string) {
	before := gameWindowPids()
	t.mu.Lock()
	defer t.mu.Unlock()
	kept := t.launched[:0]
	for _, r := range t.launched {
		if r.UUID != uuid {
			kept = append(kept, r)
		}
	}
	t.launched = append(kept, launchRec{UUID: uuid, At: time.Now(), Before: before})
	t.persist()
}

func (t *GameTracker) Forget(uuid string) {
	t.mu.Lock()
	defer t.mu.Unlock()
	kept := t.launched[:0]
	for _, r := range t.launched {
		if r.UUID != uuid {
			kept = append(kept, r)
		}
	}
	t.launched = kept
	t.persist()
}

func gameWindowPids() []uint32 {
	titles := platform.WindowTitlesByPID()
	javaPids := javaProcessPids()

	var pids []uint32
	for pid := range titles {
		if !javaPids[pid] {
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

func (t *GameTracker) Resolve() (map[string]uint32, map[string]bool) {
	pids := gameWindowPids()
	live := map[uint32]bool{}
	for _, p := range pids {
		live[p] = true
	}

	t.mu.Lock()
	defer t.mu.Unlock()

	sort.SliceStable(t.launched, func(i, j int) bool { return t.launched[i].At.Before(t.launched[j].At) })

	running := map[string]uint32{}
	claimed := map[uint32]bool{}
	changed := false

	for i := range t.launched {
		r := &t.launched[i]
		if r.Pid != 0 && live[r.Pid] && !claimed[r.Pid] {
			claimed[r.Pid] = true
			running[r.UUID] = r.Pid
		} else if r.Pid != 0 {
			r.Pid = 0
			changed = true
		}
	}

	var free []uint32
	for _, p := range pids {
		if !claimed[p] {
			free = append(free, p)
		}
	}

	for i := range t.launched {
		r := &t.launched[i]
		if r.Pid != 0 {
			continue
		}
		skip := map[uint32]bool{}
		for _, p := range r.Before {
			skip[p] = true
		}
		for fi := range free {
			p := free[fi]
			if p == 0 || claimed[p] || skip[p] {
				continue
			}
			r.Pid = p
			claimed[p] = true
			running[r.UUID] = p
			changed = true
			break
		}
	}

	launching := map[string]bool{}
	kept := t.launched[:0]
	for _, r := range t.launched {
		if r.Pid != 0 {
			kept = append(kept, r)
			continue
		}
		if time.Since(r.At) < launchingWindow {
			launching[r.UUID] = true
			kept = append(kept, r)
		} else {
			changed = true
		}
	}
	t.launched = kept

	if changed {
		t.persist()
	}

	return running, launching
}
