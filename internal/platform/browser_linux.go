package platform

import (
	"os"
	"os/exec"
)

func openExternal(link string) {
	cmd := exec.Command("xdg-open", link)
	cmd.Env = os.Environ()
	_ = cmd.Start()
}

func OpenBrowser(url, profileDir string) {
	if browser := findChromiumBrowser(); browser != "" {
		_ = os.MkdirAll(profileDir, 0o755)
		cmd := exec.Command(browser,
			"--app="+url,
			"--user-data-dir="+profileDir,
			"--window-size=1180,820",
			"--no-first-run",
			"--no-default-browser-check",
		)
		if cmd.Start() == nil {
			return
		}
	}
	openExternal(url)
}

func findChromiumBrowser() string {
	candidates := []string{
		"chromium",
		"chromium-browser",
		"google-chrome",
		"google-chrome-stable",
		"brave-browser",
		"microsoft-edge",
		"vivaldi",
	}
	for _, c := range candidates {
		if path, err := exec.LookPath(c); err == nil {
			return path
		}
	}
	return ""
}
