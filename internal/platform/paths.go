package platform

import (
	"os"
	"path/filepath"
)

type Paths struct {
	Cristalix        string
	LauncherCfg      string
	Updates          string
	Data             string
	LauncherExe      string
	Vault            string
	WebProfile       string
	Profiles         string
	Session          string
	StaffLauncherExe string
	LauncherJar      string
	Config           string
	Logs             string
}

func Resolve() Paths {
	home, _ := os.UserHomeDir()
	cristalix := filepath.Join(home, ".cristalix")

	data := dataDir()

	return Paths{
		Cristalix:        cristalix,
		LauncherCfg:      filepath.Join(cristalix, ".launcher"),
		Updates:          filepath.Join(cristalix, "updates"),
		Data:             data,
		LauncherExe:      filepath.Join(data, "Cristalix.exe"),
		Vault:            filepath.Join(data, "vault.dat"),
		WebProfile:       filepath.Join(data, "window"),
		Profiles:         filepath.Join(data, "profiles"),
		Session:          filepath.Join(data, "session.json"),
		StaffLauncherExe: filepath.Join(data, "CristalixLauncher.exe"),
		LauncherJar:      filepath.Join(data, "Cristalix.jar"),
		Config:           filepath.Join(data, "config.json"),
		Logs:             filepath.Join(data, "logs.json"),
	}
}

func (p Paths) Ensure() error {
	return os.MkdirAll(p.Data, 0o755)
}
