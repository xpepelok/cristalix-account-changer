//go:build linux && !cgo

package platform

func RunNativeWindow(url, dataPath string, iconPNG []byte, onReady func(focus, quit func())) bool {
	return false
}

func PickExecutable() string { return "" }
