package update

import (
	"os"
	"strings"
)

var variantMarkers = []string{"webkit40", "nogui"}

func assetMatches(name string) bool {
	low := strings.ToLower(name)
	if strings.HasSuffix(low, ".exe") || !strings.Contains(low, "linux") {
		return false
	}

	for _, marker := range variantMarkers {
		if marker == assetVariant {
			continue
		}
		if strings.Contains(low, marker) {
			return false
		}
	}

	return assetVariant == "" || strings.Contains(low, assetVariant)
}

func finalizeBinary(path string) error {
	return os.Chmod(path, 0o755)
}
