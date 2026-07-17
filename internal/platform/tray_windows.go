package platform

import (
	"unsafe"

	"golang.org/x/sys/windows"
)

var shell32 = windows.NewLazySystemDLL("shell32.dll")
var procShellNotify = shell32.NewProc("Shell_NotifyIconW")
var dwmapi = windows.NewLazySystemDLL("dwmapi.dll")
var procDwmSetWindowAttribute = dwmapi.NewProc("DwmSetWindowAttribute")
var procSetWindowLongPtr = user32Win.NewProc("SetWindowLongPtrW")
var procCallWindowProc = user32Win.NewProc("CallWindowProcW")
var procLoadIcon = user32Win.NewProc("LoadIconW")
var procLoadImage = user32Win.NewProc("LoadImageW")
var procGetSystemMetrics = user32Win.NewProc("GetSystemMetrics")
var procGetCursorPos = user32Win.NewProc("GetCursorPos")
var procPostMessage = user32Win.NewProc("PostMessageW")
var procDestroyWindow = user32Win.NewProc("DestroyWindow")
var procGetModuleHandle = kernel32Local.NewProc("GetModuleHandleW")
var procGetWindowRect = user32Win.NewProc("GetWindowRect")
var procGetWindowLongPtr = user32Win.NewProc("GetWindowLongPtrW")
var procSetWindowPos = user32Win.NewProc("SetWindowPos")
var procSendMessage = user32Win.NewProc("SendMessageW")
var procReleaseCapture = user32Win.NewProc("ReleaseCapture")
var procMonitorFromWindow = user32Win.NewProc("MonitorFromWindow")
var procGetMonitorInfo = user32Win.NewProc("GetMonitorInfoW")
var procIsZoomed = user32Win.NewProc("IsZoomed")
var procGetDpiForWindow = user32Win.NewProc("GetDpiForWindow")

const (
	minClientWidth  = 810
	minClientHeight = 600
)

const (
	wmClose  = 0x0010
	wmApp    = 0x8000
	wmTray   = wmApp + 1
	wmDoQuit = wmApp + 2

	wmLButtonUp     = 0x0202
	wmLButtonDblclk = 0x0203
	wmRButtonUp     = 0x0205

	swHide     = 0
	swShow     = 5
	swMinimize = 6

	wmNCCalcSize    = 0x0083
	wmNCHitTest     = 0x0084
	wmNCLButtonDown = 0x00A1

	wmGetMinMaxInfo = 0x0024

	gwlStyle      = -16
	wsMaximizeBox = 0x00010000
	wsThickFrame  = 0x00040000
	wsCaption     = 0x00C00000

	swMaximize              = 3
	monitorDefaultToNearest = 2

	swpFrameChanged = 0x0020
	swpNoMove       = 0x0002
	swpNoSize       = 0x0001
	swpNoZOrder     = 0x0004

	htClient      = 1
	htCaption     = 2
	htLeft        = 10
	htRight       = 11
	htTop         = 12
	htTopLeft     = 13
	htTopRight    = 14
	htBottom      = 15
	htBottomLeft  = 16
	htBottomRight = 17

	resizeBorder = 8

	nimAdd    = 0x0
	nimDelete = 0x2

	nifMessage = 0x1
	nifIcon    = 0x2
	nifTip     = 0x4

	dwmwaWindowCornerPreference = 33
	dwmwaBorderColor            = 34
	dwmwcpRound                 = 2
	appBorderColor              = 0x00423C38
)

type notifyIconData struct {
	CbSize           uint32
	HWnd             uintptr
	UID              uint32
	UFlags           uint32
	UCallbackMessage uint32
	HIcon            uintptr
	SzTip            [128]uint16
	DwState          uint32
	DwStateMask      uint32
	SzInfo           [256]uint16
	UVersion         uint32
	SzInfoTitle      [64]uint16
	DwInfoFlags      uint32
	GuidItem         windows.GUID
	HBalloonIcon     uintptr
}

type point struct{ X, Y int32 }

type winRect struct{ Left, Top, Right, Bottom int32 }

type monitorInfo struct {
	Size    uint32
	Monitor winRect
	Work    winRect
	Flags   uint32
}

type minMaxInfo struct {
	Reserved     point
	MaxSize      point
	MaxPosition  point
	MinTrackSize point
	MaxTrackSize point
}

var trayOldProc uintptr
var trayData notifyIconData
var trayProcRef = windows.NewCallback(trayWndProc)

func setupTray(hwnd uintptr) {
	idx := int32(-4)
	old, _, _ := procSetWindowLongPtr.Call(hwnd, uintptr(idx), trayProcRef)
	trayOldProc = old

	makeFrameless(hwnd)

	hinst, _, _ := procGetModuleHandle.Call(0)
	const smCXSMICON, smCYSMICON = 49, 50
	const imageIcon, lrDefaultColor = 1, 0
	cx, _, _ := procGetSystemMetrics.Call(smCXSMICON)
	cy, _, _ := procGetSystemMetrics.Call(smCYSMICON)
	hicon, _, _ := procLoadImage.Call(hinst, 1, imageIcon, cx, cy, lrDefaultColor)
	if hicon == 0 {
		hicon, _, _ = procLoadIcon.Call(hinst, 1)
	}

	trayData = notifyIconData{
		CbSize:           uint32(unsafe.Sizeof(notifyIconData{})),
		HWnd:             hwnd,
		UID:              1,
		UFlags:           nifMessage | nifIcon | nifTip,
		UCallbackMessage: wmTray,
		HIcon:            hicon,
	}
	tip := windows.StringToUTF16("AccountChanger")
	copy(trayData.SzTip[:], tip)

	procShellNotify.Call(nimAdd, uintptr(unsafe.Pointer(&trayData)))
}

func makeFrameless(hwnd uintptr) {
	si := int32(gwlStyle)
	style, _, _ := procGetWindowLongPtr.Call(hwnd, uintptr(si))
	style |= wsMaximizeBox | wsThickFrame | wsCaption
	procSetWindowLongPtr.Call(hwnd, uintptr(si), style)
	procSetWindowPos.Call(hwnd, 0, 0, 0, 0, 0, swpFrameChanged|swpNoMove|swpNoSize|swpNoZOrder)
	applyWindowChrome(hwnd)
}

func applyWindowChrome(hwnd uintptr) {
	pref := int32(dwmwcpRound)
	procDwmSetWindowAttribute.Call(hwnd, dwmwaWindowCornerPreference, uintptr(unsafe.Pointer(&pref)), unsafe.Sizeof(pref))
	color := uint32(appBorderColor)
	procDwmSetWindowAttribute.Call(hwnd, dwmwaBorderColor, uintptr(unsafe.Pointer(&color)), unsafe.Sizeof(color))
}

func maximizeToggle(hwnd uintptr) {
	if z, _, _ := procIsZoomed.Call(hwnd); z != 0 {
		procShowWindow.Call(hwnd, swRestore)
		return
	}
	procShowWindow.Call(hwnd, swMaximize)
}

func fillMaxInfo(hwnd, lparam uintptr) {
	hmon, _, _ := procMonitorFromWindow.Call(hwnd, monitorDefaultToNearest)
	if hmon == 0 {
		return
	}
	var mi monitorInfo
	mi.Size = uint32(unsafe.Sizeof(mi))
	if r, _, _ := procGetMonitorInfo.Call(hmon, uintptr(unsafe.Pointer(&mi))); r == 0 {
		return
	}
	mmi := (*minMaxInfo)(unsafe.Pointer(lparam))
	mmi.MaxPosition.X = mi.Work.Left - mi.Monitor.Left
	mmi.MaxPosition.Y = mi.Work.Top - mi.Monitor.Top
	mmi.MaxSize.X = mi.Work.Right - mi.Work.Left
	mmi.MaxSize.Y = mi.Work.Bottom - mi.Work.Top
	mmi.MaxTrackSize.X = mmi.MaxSize.X
	mmi.MaxTrackSize.Y = mmi.MaxSize.Y
	dpi := uintptr(96)
	if d, _, _ := procGetDpiForWindow.Call(hwnd); d != 0 {
		dpi = d
	}
	mmi.MinTrackSize.X = int32(minClientWidth * dpi / 96)
	mmi.MinTrackSize.Y = int32(minClientHeight * dpi / 96)
}

func hitTest(hwnd, lparam uintptr) uintptr {
	x := int32(int16(uint16(lparam & 0xffff)))
	y := int32(int16(uint16((lparam >> 16) & 0xffff)))
	var rect winRect
	procGetWindowRect.Call(hwnd, uintptr(unsafe.Pointer(&rect)))

	left := x < rect.Left+resizeBorder
	right := x >= rect.Right-resizeBorder
	top := y < rect.Top+resizeBorder
	bottom := y >= rect.Bottom-resizeBorder

	switch {
	case top && left:
		return htTopLeft
	case top && right:
		return htTopRight
	case bottom && left:
		return htBottomLeft
	case bottom && right:
		return htBottomRight
	case left:
		return htLeft
	case right:
		return htRight
	case top:
		return htTop
	case bottom:
		return htBottom
	}
	return htClient
}

func startDrag(hwnd uintptr) {
	procReleaseCapture.Call()
	procSendMessage.Call(hwnd, wmNCLButtonDown, htCaption, 0)
}

func startResize(hwnd, code uintptr) {
	if z, _, _ := procIsZoomed.Call(hwnd); z != 0 {
		return
	}
	if code < htLeft || code > htBottomRight {
		return
	}
	procReleaseCapture.Call()
	procSendMessage.Call(hwnd, wmNCLButtonDown, code, 0)
}

func minimizeWindow(hwnd uintptr) {
	procShowWindow.Call(hwnd, swMinimize)
}

func trayWndProc(hwnd, msg, wparam, lparam uintptr) uintptr {
	switch msg {
	case wmNCCalcSize:
		if wparam != 0 {
			return 0
		}
	case wmNCHitTest:
		return hitTest(hwnd, lparam)
	case wmGetMinMaxInfo:
		fillMaxInfo(hwnd, lparam)
		return 0
	case wmClose:
		procShowWindow.Call(hwnd, swHide)
		TrimMemory()
		return 0
	case wmDoQuit:
		quitApp(hwnd)
		return 0
	case wmTray:
		switch lparam {
		case wmLButtonUp, wmLButtonDblclk:
			restoreWindow(hwnd)
		case wmRButtonUp:
			showTrayMenu(hwnd)
		}
		return 0
	}
	r, _, _ := procCallWindowProc.Call(trayOldProc, hwnd, msg, wparam, lparam)
	return r
}

func restoreWindow(hwnd uintptr) {
	procShowWindow.Call(hwnd, swShow)
	procShowWindow.Call(hwnd, swRestore)
	procSetForeground.Call(hwnd)
}

func showTrayMenu(hwnd uintptr) {
	showTrayMenuCustom(hwnd)
}

func requestQuit(hwnd uintptr) {
	procPostMessage.Call(hwnd, wmDoQuit, 0, 0)
}

func quitApp(hwnd uintptr) {
	procShellNotify.Call(nimDelete, uintptr(unsafe.Pointer(&trayData)))
	procDestroyWindow.Call(hwnd)
}
