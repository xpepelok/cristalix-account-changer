package app

import (
	"accountchanger/internal/config"
	"errors"
	"net/http"
	"runtime"
)

var errNoUIAutomation = errors.New("импорт по логину/паролю и обновление токенов недоступны на Linux: войди в аккаунт через лаунчер, токен подхватится автоматически")

type Caps struct {
	OS               string `json:"os"`
	CredentialImport bool   `json:"credentialImport"`
	AutoPlay         bool   `json:"autoPlay"`
	ExeLaunchers     bool   `json:"exeLaunchers"`
	Tray             bool   `json:"tray"`
}

func caps() Caps {
	return Caps{
		OS:               runtime.GOOS,
		CredentialImport: credentialImportSupported,
		AutoPlay:         autoPlaySupported,
		ExeLaunchers:     exeLaunchersSupported,
		Tray:             traySupported,
	}
}

func (s *Server) handleCaps(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, http.StatusOK, caps())
}

func LauncherAllowed(v string) bool {
	if exeLaunchersSupported {
		return true
	}
	return v != config.LauncherNormal && v != config.LauncherNew
}
