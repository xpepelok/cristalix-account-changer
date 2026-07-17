package update

import "testing"

var releaseAssets = []string{
	"AccountChanger.exe",
	"AccountChanger-linux-x86_64",
	"checksums.txt",
}

func TestAssetMatchesPicksExactlyOneExe(t *testing.T) {
	var picked []string
	for _, a := range releaseAssets {
		if assetMatches(a) {
			picked = append(picked, a)
		}
	}
	if len(picked) != 1 {
		t.Fatalf("matched %d assets (%v), want exactly 1", len(picked), picked)
	}
	if picked[0] != "AccountChanger.exe" {
		t.Fatalf("picked %q, want AccountChanger.exe", picked[0])
	}
}

func TestAssetMatchesRejectsNonExe(t *testing.T) {
	for _, name := range []string{
		"AccountChanger-linux-x86_64",
		"checksums.txt",
		"Source code (zip)",
		"AccountChanger.zip",
		"AccountChanger-setup.msi",
	} {
		if assetMatches(name) {
			t.Errorf("assetMatches(%q) = true, want false", name)
		}
	}
}
