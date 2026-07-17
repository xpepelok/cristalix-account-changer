package platform

import (
	"os"
	"path/filepath"
)

func InstallDesktopEntry(iconPNG []byte) {
	exe, err := os.Executable()
	if err != nil {
		return
	}
	dataHome := xdgDataHome()

	if len(iconPNG) > 0 {
		iconDir := filepath.Join(dataHome, "icons", "hicolor", "256x256", "apps")
		if os.MkdirAll(iconDir, 0o755) == nil {
			_ = os.WriteFile(filepath.Join(iconDir, "accountchanger.png"), iconPNG, 0o644)
		}
	}

	appsDir := filepath.Join(dataHome, "applications")
	if os.MkdirAll(appsDir, 0o755) != nil {
		return
	}
	entry := "[Desktop Entry]\n" +
		"Type=Application\n" +
		"Name=AccountChanger\n" +
		"Comment=Менеджер аккаунтов Cristalix\n" +
		"Exec=" + quoteExec(exe) + "\n" +
		"Icon=accountchanger\n" +
		"Terminal=false\n" +
		"Categories=Utility;\n" +
		"StartupWMClass=Accountchanger\n"
	_ = os.WriteFile(filepath.Join(appsDir, "accountchanger.desktop"), []byte(entry), 0o644)
}
