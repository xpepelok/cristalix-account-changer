package platform

import (
	"os"
	"os/exec"
	"path/filepath"

	"github.com/jchv/go-webview2"
	"golang.org/x/sys/windows"
)

var user32Win = windows.NewLazySystemDLL("user32.dll")
var procSetForeground = user32Win.NewProc("SetForegroundWindow")
var procShowWindow = user32Win.NewProc("ShowWindow")

const swRestore = 9

func RunNativeWindow(url, dataPath string, onReady func(focus, quit func())) (ran bool) {
	defer func() {
		if recover() != nil {
			ran = false
		}
	}()

	_ = os.MkdirAll(dataPath, 0o755)

	_ = os.Setenv("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS",
		"--enable-low-end-device-mode --in-process-gpu --renderer-process-limit=1 --js-flags=--max-old-space-size=48 "+
			"--disable-features=Translate,AutofillServerCommunication,OptimizationHints,MediaRouter,BackForwardCache,InterestFeedContentSuggestions,CalculateNativeWinOcclusion,AudioServiceOutOfProcess "+
			"--disable-background-networking --disable-gpu-shader-disk-cache --disk-cache-size=1048576 --media-cache-size=1048576")

	view := webview2.NewWithOptions(webview2.WebViewOptions{
		Debug:     false,
		AutoFocus: true,
		DataPath:  dataPath,
		WindowOptions: webview2.WindowOptions{
			Title:  "AccountChanger",
			Width:  1180,
			Height: 820,
			IconId: 1,
			Center: true,
		},
	})
	if view == nil {
		return false
	}
	defer view.Destroy()

	raw := view.Window()
	if raw == nil {
		return false
	}
	hwnd := uintptr(raw)

	setupTray(hwnd)

	if onReady != nil {
		onReady(func() { restoreWindow(hwnd) }, func() { requestQuit(hwnd) })
	}

	view.Bind("acQuit", func() {
		requestQuit(hwnd)
	})
	view.Bind("acMinimize", func() {
		minimizeWindow(hwnd)
	})
	view.Bind("acHide", func() {
		procShowWindow.Call(hwnd, swHide)
		TrimMemory()
	})
	view.Bind("acDrag", func() {
		startDrag(hwnd)
	})
	view.Bind("acResize", func(raw ...any) {
		if len(raw) > 0 {
			if f, ok := raw[0].(float64); ok {
				startResize(hwnd, uintptr(int(f)))
			}
		}
	})
	view.Bind("acMaximize", func() {
		maximizeToggle(hwnd)
	})
	view.Bind("acOpenUrl", func(raw ...any) {
		if len(raw) > 0 {
			if link, ok := raw[0].(string); ok {
				openExternal(link)
			}
		}
	})
	view.Bind("acCopy", func(raw ...any) bool {
		if len(raw) > 0 {
			if text, ok := raw[0].(string); ok {
				return setClipboard(text)
			}
		}
		return false
	})
	view.Bind("acPickLauncher", func() string {
		return PickExecutable()
	})
	view.Navigate(url)
	view.Run()
	return true
}

func openExternal(link string) {
	_ = exec.Command("rundll32", "url.dll,FileProtocolHandler", link).Start()
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
	_ = exec.Command("rundll32", "url.dll,FileProtocolHandler", url).Start()
}

func findChromiumBrowser() string {
	candidates := []string{
		filepath.Join(os.Getenv("ProgramFiles(x86)"), "Microsoft", "Edge", "Application", "msedge.exe"),
		filepath.Join(os.Getenv("ProgramFiles"), "Microsoft", "Edge", "Application", "msedge.exe"),
		filepath.Join(os.Getenv("ProgramFiles"), "Google", "Chrome", "Application", "chrome.exe"),
		filepath.Join(os.Getenv("ProgramFiles(x86)"), "Google", "Chrome", "Application", "chrome.exe"),
		filepath.Join(os.Getenv("LOCALAPPDATA"), "Google", "Chrome", "Application", "chrome.exe"),
	}
	for _, c := range candidates {
		if c == "" {
			continue
		}
		if _, err := os.Stat(c); err == nil {
			return c
		}
	}
	return ""
}
