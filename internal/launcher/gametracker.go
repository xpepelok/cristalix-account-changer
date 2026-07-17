package launcher

import (
	"accountchanger/internal/vault"
	"encoding/json"
	"os"
	"sort"
	"sync"
	"time"
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

func (t *GameTracker) bindGame(uuid string, before []uint32) uint32 {
	beforeSet := map[uint32]bool{}
	for _, p := range before {
		beforeSet[p] = true
	}
	deadline := time.Now().Add(40 * time.Second)
	for {
		pids := gameWindowPids()
		t.mu.Lock()
		claimed := map[uint32]bool{}
		for _, r := range t.launched {
			if r.Pid != 0 {
				claimed[r.Pid] = true
			}
		}
		var found uint32
		for _, p := range pids {
			if !beforeSet[p] && !claimed[p] {
				found = p
				break
			}
		}
		if found != 0 {
			set := false
			for i := range t.launched {
				if t.launched[i].UUID == uuid {
					t.launched[i].Pid = found
					t.launched[i].At = time.Now()
					set = true
					break
				}
			}
			if !set {
				t.launched = append(t.launched, launchRec{UUID: uuid, At: time.Now(), Pid: found})
			}
			t.persist()
			t.mu.Unlock()
			return found
		}
		t.mu.Unlock()
		if time.Now().After(deadline) {
			return 0
		}
		time.Sleep(600 * time.Millisecond)
	}
}

func (t *GameTracker) bindVerifiedLauncher(uuid string, launcherPID uint32) uint32 {
	deadline := time.Now().Add(90 * time.Second)
	for {
		children := javaDescendantsOf(launcherPID)
		for _, pid := range gameWindowPids() {
			if !children[pid] {
				continue
			}
			t.mu.Lock()
			for i := range t.launched {
				if t.launched[i].UUID == uuid {
					t.launched[i].Pid = pid
					t.launched[i].At = time.Now()
					t.persist()
					t.mu.Unlock()
					return pid
				}
			}
			t.mu.Unlock()
		}
		if time.Now().After(deadline) {
			return 0
		}
		time.Sleep(300 * time.Millisecond)
	}
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
