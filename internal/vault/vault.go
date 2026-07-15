package vault

import (
	"accountchanger/internal/jwt"
	"accountchanger/internal/platform"
	"encoding/json"
	"os"
	"sort"
	"sync"
	"time"
)

type Account struct {
	UUID           string `json:"uuid"`
	Name           string `json:"name"`
	Token          string `json:"token"`
	Label          string `json:"label"`
	Pinned         bool   `json:"pinned"`
	Client         string `json:"client"`
	Profile        string `json:"profile"`
	Ram            int    `json:"ram"`
	MinGraphics    bool   `json:"minGraphics"`
	Fullscreen     bool   `json:"fullscreen"`
	DiscordRPC     bool   `json:"discordRPC"`
	AutoEnter      bool   `json:"autoEnter"`
	DebugMode      bool   `json:"debugMode"`
	RenderDistance int    `json:"renderDistance"`
	MaxFps         int    `json:"maxFps"`
	Animations     int    `json:"animations"`
	FastRender     int    `json:"fastRender"`
	Expires        int64  `json:"expires"`
	FirstSeen      int64  `json:"firstSeen"`
	LastSeen       int64  `json:"lastSeen"`
	LastLaunched   int64  `json:"lastLaunched"`
}

type LaunchOpts struct {
	Ram            int
	MinGraphics    bool
	Fullscreen     bool
	DiscordRPC     bool
	AutoEnter      bool
	DebugMode      bool
	RenderDistance int
	MaxFps         int
	Animations     int
	FastRender     int
}

type Group struct {
	ID      string   `json:"id"`
	Name    string   `json:"name"`
	Members []string `json:"members"`
	Profile string   `json:"profile"`
	Pinned  bool     `json:"pinned"`
	Order   int      `json:"order"`
}

type Vault struct {
	mu        sync.Mutex
	path      string
	Accounts  map[string]*Account `json:"accounts"`
	Forgotten map[string]string   `json:"forgotten"`
	Groups    []*Group            `json:"groups"`
}

type vaultData struct {
	Accounts  map[string]*Account `json:"accounts"`
	Forgotten map[string]string   `json:"forgotten"`
	Groups    []*Group            `json:"groups"`
}

func OpenVault(path string) *Vault {
	v := &Vault{path: path, Accounts: map[string]*Account{}, Forgotten: map[string]string{}}
	raw, err := os.ReadFile(path)
	if err != nil {
		return v
	}
	plain, err := platform.DPAPIDecrypt(raw)
	if err != nil {
		return v
	}
	var stored vaultData
	if json.Unmarshal(plain, &stored) == nil {
		if stored.Accounts != nil {
			v.Accounts = stored.Accounts
		}
		if stored.Forgotten != nil {
			v.Forgotten = stored.Forgotten
		}
		v.Groups = stored.Groups
	}
	return v
}

func (v *Vault) persist() error {
	payload, err := json.Marshal(vaultData{Accounts: v.Accounts, Forgotten: v.Forgotten, Groups: v.Groups})
	if err != nil {
		return err
	}
	enc, err := platform.DPAPIEncrypt(payload)
	if err != nil {
		return err
	}
	tmp := v.path + ".tmp"
	if err := os.WriteFile(tmp, enc, 0o600); err != nil {
		return err
	}
	return os.Rename(tmp, v.path)
}

func (v *Vault) UpsertToken(name, token string, claims jwt.Claims) bool {
	v.mu.Lock()
	defer v.mu.Unlock()

	if old, ok := v.Forgotten[claims.UUID]; ok {
		if old == token {
			return false
		}
		delete(v.Forgotten, claims.UUID)
	}

	now := time.Now().Unix()
	acc, ok := v.Accounts[claims.UUID]
	if !ok {
		acc = &Account{UUID: claims.UUID, FirstSeen: now}
		v.Accounts[claims.UUID] = acc
	}
	changed := false
	if name != "" && acc.Name != name {
		acc.Name = name
		changed = true
	}
	if token != "" && acc.Token != token {
		acc.Token = token
		acc.Expires = claims.Exp
		changed = true
	}
	acc.LastSeen = now
	if changed {
		_ = v.persist()
	}
	return changed
}

func (v *Vault) SetLabel(uuid, label string) {
	v.mu.Lock()
	defer v.mu.Unlock()
	if acc, ok := v.Accounts[uuid]; ok {
		acc.Label = label
		_ = v.persist()
	}
}

func (v *Vault) SetPinned(uuid string, pinned bool) {
	v.mu.Lock()
	defer v.mu.Unlock()
	if acc, ok := v.Accounts[uuid]; ok {
		acc.Pinned = pinned
		_ = v.persist()
	}
}

func (v *Vault) SetClient(uuid, client string) {
	v.mu.Lock()
	defer v.mu.Unlock()
	if acc, ok := v.Accounts[uuid]; ok && client != "" {
		acc.Client = client
		_ = v.persist()
	}
}

func (v *Vault) SetProfile(uuid, profile string) {
	v.mu.Lock()
	defer v.mu.Unlock()
	if acc, ok := v.Accounts[uuid]; ok {
		acc.Profile = profile
		_ = v.persist()
	}
}

func (v *Vault) SetRam(uuids []string, ram int) {
	v.mu.Lock()
	defer v.mu.Unlock()
	changed := false
	for _, uuid := range uuids {
		if acc, ok := v.Accounts[uuid]; ok {
			acc.Ram = ram
			changed = true
		}
	}
	if changed {
		_ = v.persist()
	}
}

func (v *Vault) SetLaunchSettings(uuids []string, opts LaunchOpts) {
	v.mu.Lock()
	defer v.mu.Unlock()
	changed := false
	for _, uuid := range uuids {
		if acc, ok := v.Accounts[uuid]; ok {
			acc.Ram = opts.Ram
			acc.MinGraphics = opts.MinGraphics
			acc.Fullscreen = opts.Fullscreen
			acc.DiscordRPC = opts.DiscordRPC
			acc.AutoEnter = opts.AutoEnter
			acc.DebugMode = opts.DebugMode
			acc.RenderDistance = opts.RenderDistance
			acc.MaxFps = opts.MaxFps
			acc.Animations = opts.Animations
			acc.FastRender = opts.FastRender
			changed = true
		}
	}
	if changed {
		_ = v.persist()
	}
}

func (v *Vault) MarkLaunched(uuid string) {
	v.mu.Lock()
	defer v.mu.Unlock()
	if acc, ok := v.Accounts[uuid]; ok {
		acc.LastLaunched = time.Now().Unix()
		_ = v.persist()
	}
}

func (v *Vault) Forget(uuid string) {
	v.mu.Lock()
	defer v.mu.Unlock()
	token := ""
	if acc, ok := v.Accounts[uuid]; ok {
		token = acc.Token
	}
	v.Forgotten[uuid] = token
	delete(v.Accounts, uuid)
	for _, g := range v.Groups {
		g.Members = removeString(g.Members, uuid)
	}
	_ = v.persist()
}

func (v *Vault) Get(uuid string) (*Account, bool) {
	v.mu.Lock()
	defer v.mu.Unlock()
	acc, ok := v.Accounts[uuid]
	if !ok {
		return nil, false
	}
	clone := *acc
	return &clone, true
}

func (v *Vault) List() []*Account {
	v.mu.Lock()
	defer v.mu.Unlock()
	out := make([]*Account, 0, len(v.Accounts))
	for _, acc := range v.Accounts {
		clone := *acc
		out = append(out, &clone)
	}
	sort.Slice(out, func(i, j int) bool {
		if out[i].Pinned != out[j].Pinned {
			return out[i].Pinned
		}
		if out[i].LastLaunched != out[j].LastLaunched {
			return out[i].LastLaunched > out[j].LastLaunched
		}
		if out[i].LastSeen != out[j].LastSeen {
			return out[i].LastSeen > out[j].LastSeen
		}
		return out[i].Name < out[j].Name
	})
	return out
}
