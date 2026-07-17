package update

import (
	"path/filepath"
	"strings"
)

func assetMatches(name string) bool {
	return strings.EqualFold(filepath.Ext(name), ".exe")
}

func finalizeBinary(path string) error { return nil }
