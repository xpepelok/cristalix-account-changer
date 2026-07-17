package update

import "testing"

var releaseAssets = []string{
	"AccountChanger.exe",
	"AccountChanger-linux-x86_64",
	"AccountChanger-linux-x86_64-webkit40",
	"AccountChanger-linux-x86_64-nogui",
}

func TestAssetMatchesPicksExactlyOne(t *testing.T) {
	var picked []string
	for _, a := range releaseAssets {
		if assetMatches(a) {
			picked = append(picked, a)
		}
	}
	if len(picked) != 1 {
		t.Fatalf("assetVariant=%q matched %d assets (%v), want exactly 1", assetVariant, len(picked), picked)
	}

	want := map[string]string{
		"":         "AccountChanger-linux-x86_64",
		"webkit40": "AccountChanger-linux-x86_64-webkit40",
		"nogui":    "AccountChanger-linux-x86_64-nogui",
	}[assetVariant]
	if picked[0] != want {
		t.Fatalf("assetVariant=%q picked %q, want %q", assetVariant, picked[0], want)
	}
}

func TestAssetMatchesRejectsWindowsAndNoise(t *testing.T) {
	for _, name := range []string{
		"AccountChanger.exe",
		"AccountChanger-linux-x86_64.exe",
		"checksums.txt",
		"Source code (zip)",
		"AccountChanger-darwin-arm64",
	} {
		if assetMatches(name) {
			t.Errorf("assetMatches(%q) = true, want false", name)
		}
	}
}

func TestDefaultVariantRejectsMarkedAssets(t *testing.T) {
	if assetVariant != "" {
		t.Skip("only meaningful for the unmarked default build")
	}
	for _, name := range []string{
		"AccountChanger-linux-x86_64-webkit40",
		"AccountChanger-linux-x86_64-nogui",
	} {
		if assetMatches(name) {
			t.Errorf("default build accepted %q", name)
		}
	}
}
