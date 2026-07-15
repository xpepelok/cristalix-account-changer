package vault

import (
	"crypto/rand"
	"encoding/hex"
	"errors"
	"sort"
	"strings"
)

func newGroupID() string {
	b := make([]byte, 8)
	if _, err := rand.Read(b); err != nil {
		return "g"
	}
	return "g" + hex.EncodeToString(b)
}

func removeString(list []string, val string) []string {
	out := list[:0]
	for _, s := range list {
		if s != val {
			out = append(out, s)
		}
	}
	return out
}

func containsString(list []string, val string) bool {
	for _, s := range list {
		if s == val {
			return true
		}
	}
	return false
}

func (v *Vault) SortedGroups() []*Group {
	v.mu.Lock()
	defer v.mu.Unlock()
	out := make([]*Group, 0, len(v.Groups))
	for _, g := range v.Groups {
		clone := *g
		clone.Members = append([]string(nil), g.Members...)
		out = append(out, &clone)
	}
	sort.SliceStable(out, func(i, j int) bool {
		if out[i].Pinned != out[j].Pinned {
			return out[i].Pinned
		}
		return out[i].Order < out[j].Order
	})
	return out
}

func (v *Vault) findGroup(id string) *Group {
	for _, g := range v.Groups {
		if g.ID == id {
			return g
		}
	}
	return nil
}

func (v *Vault) CreateGroup(name string) (*Group, error) {
	name = strings.TrimSpace(name)
	if name == "" {
		return nil, errors.New("пустое название")
	}
	if r := []rune(name); len(r) > 24 {
		name = string(r[:24])
	}
	v.mu.Lock()
	defer v.mu.Unlock()
	maxOrder := 0
	for _, g := range v.Groups {
		if g.Order >= maxOrder {
			maxOrder = g.Order + 1
		}
	}
	g := &Group{ID: newGroupID(), Name: name, Members: []string{}, Order: maxOrder}
	v.Groups = append(v.Groups, g)
	_ = v.persist()
	clone := *g
	return &clone, nil
}

func (v *Vault) DeleteGroup(id string) {
	v.mu.Lock()
	defer v.mu.Unlock()
	kept := v.Groups[:0]
	for _, g := range v.Groups {
		if g.ID != id {
			kept = append(kept, g)
		}
	}
	v.Groups = kept
	_ = v.persist()
}

func (v *Vault) RenameGroup(id, name string) error {
	name = strings.TrimSpace(name)
	if name == "" {
		return errors.New("пустое название")
	}
	if r := []rune(name); len(r) > 24 {
		name = string(r[:24])
	}
	v.mu.Lock()
	defer v.mu.Unlock()
	g := v.findGroup(id)
	if g == nil {
		return errors.New("группа не найдена")
	}
	g.Name = name
	_ = v.persist()
	return nil
}

func (v *Vault) SetGroupPinned(id string, pinned bool) {
	v.mu.Lock()
	defer v.mu.Unlock()
	if g := v.findGroup(id); g != nil {
		g.Pinned = pinned
		_ = v.persist()
	}
}

func (v *Vault) SetGroupProfile(id, profile string) {
	v.mu.Lock()
	defer v.mu.Unlock()
	if g := v.findGroup(id); g != nil {
		g.Profile = profile
		_ = v.persist()
	}
}

func (v *Vault) ReorderGroups(ids []string) {
	v.mu.Lock()
	defer v.mu.Unlock()
	index := map[string]int{}
	for i, id := range ids {
		index[id] = i
	}
	for _, g := range v.Groups {
		if i, ok := index[g.ID]; ok {
			g.Order = i
		}
	}
	_ = v.persist()
}

func (v *Vault) AddGroupMember(id, uuid string) {
	v.mu.Lock()
	defer v.mu.Unlock()
	g := v.findGroup(id)
	if g == nil {
		return
	}
	if _, ok := v.Accounts[uuid]; !ok {
		return
	}
	if !containsString(g.Members, uuid) {
		g.Members = append(g.Members, uuid)
		_ = v.persist()
	}
}

func (v *Vault) RemoveGroupMember(id, uuid string) {
	v.mu.Lock()
	defer v.mu.Unlock()
	if g := v.findGroup(id); g != nil {
		g.Members = removeString(g.Members, uuid)
		_ = v.persist()
	}
}

func (v *Vault) GroupMembers(id string) ([]string, string, bool) {
	v.mu.Lock()
	defer v.mu.Unlock()
	g := v.findGroup(id)
	if g == nil {
		return nil, "", false
	}
	members := make([]string, 0, len(g.Members))
	for _, uuid := range g.Members {
		if _, ok := v.Accounts[uuid]; ok {
			members = append(members, uuid)
		}
	}
	return members, g.Profile, true
}
