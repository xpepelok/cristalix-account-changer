package platform

import (
	"unsafe"

	"golang.org/x/sys/windows"
)

var shcore = windows.NewLazySystemDLL("shcore.dll")
var procSetDpiAwarenessContext = user32Win.NewProc("SetProcessDpiAwarenessContext")
var procSetProcessDpiAwareness = shcore.NewProc("SetProcessDpiAwareness")
var procSetProcessDPIAware = user32Win.NewProc("SetProcessDPIAware")
var procGetDpiForSystem = user32Win.NewProc("GetDpiForSystem")
var procSystemParametersInfo = user32Win.NewProc("SystemParametersInfoW")

const dpiAwarenessContextSystemAware = ^uintptr(1)
const processSystemDpiAware = 1
const spiGetWorkArea = 0x0030

func enableDPIAwareness() {
	if procSetDpiAwarenessContext.Find() == nil {
		if r, _, _ := procSetDpiAwarenessContext.Call(dpiAwarenessContextSystemAware); r != 0 {
			return
		}
	}
	if procSetProcessDpiAwareness.Find() == nil {
		if r, _, _ := procSetProcessDpiAwareness.Call(processSystemDpiAware); r == 0 {
			return
		}
	}
	procSetProcessDPIAware.Call()
}

func systemDPI() int {
	if procGetDpiForSystem.Find() == nil {
		if r, _, _ := procGetDpiForSystem.Call(); r >= 96 {
			return int(r)
		}
	}
	return 96
}

func workArea() (int, int) {
	var r winRect
	ok, _, _ := procSystemParametersInfo.Call(spiGetWorkArea, 0, uintptr(unsafe.Pointer(&r)), 0)
	if ok == 0 {
		return 0, 0
	}
	return int(r.Right - r.Left), int(r.Bottom - r.Top)
}

func scaledWindowSize(baseW, baseH int) (int, int) {
	dpi := systemDPI()
	w := baseW * dpi / 96
	h := baseH * dpi / 96
	maxW, maxH := workArea()
	if maxW > 0 && w > maxW {
		w = maxW
	}
	if maxH > 0 && h > maxH {
		h = maxH
	}
	return w, h
}
