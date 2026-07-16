package platform

import (
	"unsafe"

	"golang.org/x/sys/windows"
)

var gdi32 = windows.NewLazySystemDLL("gdi32.dll")
var procCreateSolidBrush = gdi32.NewProc("CreateSolidBrush")
var procDeleteObject = gdi32.NewProc("DeleteObject")
var procSelectObject = gdi32.NewProc("SelectObject")
var procRoundRect = gdi32.NewProc("RoundRect")
var procFillRect = user32Win.NewProc("FillRect")
var procCreateFontIndirect = gdi32.NewProc("CreateFontIndirectW")
var procGetStockObject = gdi32.NewProc("GetStockObject")
var procSetTextColor = gdi32.NewProc("SetTextColor")
var procSetBkMode = gdi32.NewProc("SetBkMode")
var procDrawText = user32Win.NewProc("DrawTextW")

var procRegisterClassEx = user32Win.NewProc("RegisterClassExW")
var procCreateWindowEx = user32Win.NewProc("CreateWindowExW")
var procDefWindowProc = user32Win.NewProc("DefWindowProcW")
var procBeginPaint = user32Win.NewProc("BeginPaint")
var procEndPaint = user32Win.NewProc("EndPaint")
var procGetClientRect = user32Win.NewProc("GetClientRect")
var procInvalidateRect = user32Win.NewProc("InvalidateRect")
var procSetFocus = user32Win.NewProc("SetFocus")
var procAnimateWindow = user32Win.NewProc("AnimateWindow")

const (
	wmPaint      = 0x000F
	wmDestroy    = 0x0002
	wmEraseBkgnd = 0x0014
	wmActivate   = 0x0006
	wmMouseMove  = 0x0200

	wsPopup      = 0x80000000
	wsExTopmost  = 0x00000008
	wsExToolWin  = 0x00000080
	csDropShadow = 0x00020000

	awBlend  = 0x00080000
	awCenter = 0x00000010
	awHide   = 0x00010000

	openAnimMs  = 140
	closeAnimMs = 110

	transparentBk = 1

	dtSingleLine = 0x0020
	dtVCenter    = 0x0004
	dtLeft       = 0x0000

	smCxscreen = 0
	smCyscreen = 1

	waInactive = 0

	menuPad      = 8
	menuItemH    = 34
	menuItemGap  = 2
	menuItemR    = 7
	menuWidth    = 188
	fwNormal     = 400
	defaultCharS = 1
	nullPenStock = 8
)

type wndClassExW struct {
	cbSize        uint32
	style         uint32
	lpfnWndProc   uintptr
	cbClsExtra    int32
	cbWndExtra    int32
	hInstance     uintptr
	hIcon         uintptr
	hCursor       uintptr
	hbrBackground uintptr
	lpszMenuName  *uint16
	lpszClassName *uint16
	hIconSm       uintptr
}

type paintStruct struct {
	hdc         uintptr
	fErase      int32
	rcPaint     winRect
	fRestore    int32
	fIncUpdate  int32
	rgbReserved [32]byte
}

type logFont struct {
	lfHeight         int32
	lfWidth          int32
	lfEscapement     int32
	lfOrientation    int32
	lfWeight         int32
	lfItalic         byte
	lfUnderline      byte
	lfStrikeOut      byte
	lfCharSet        byte
	lfOutPrecision   byte
	lfClipPrecision  byte
	lfQuality        byte
	lfPitchAndFamily byte
	lfFaceName       [32]uint16
}

var trayMenuClassReg bool
var trayMenuHwnd uintptr
var trayMenuOwner uintptr
var trayMenuHover int32 = -1
var trayMenuItems = []string{"Открыть", "Закрыть"}
var trayMenuProcRef = windows.NewCallback(trayMenuWndProc)

const (
	menuBg     = 0x001C1C1C
	menuText   = 0x00F0F0F0
	menuHoverB = 0x002A2A2A
)

func ensureTrayMenuClass() {
	if trayMenuClassReg {
		return
	}
	hinst, _, _ := procGetModuleHandle.Call(0)
	className := windows.StringToUTF16Ptr("ACTrayMenu")
	var wc wndClassExW
	wc.cbSize = uint32(unsafe.Sizeof(wc))
	wc.style = csDropShadow
	wc.lpfnWndProc = trayMenuProcRef
	wc.hInstance = hinst
	wc.lpszClassName = className
	procRegisterClassEx.Call(uintptr(unsafe.Pointer(&wc)))
	trayMenuClassReg = true
}

func showTrayMenuCustom(owner uintptr) {
	ensureTrayMenuClass()
	trayMenuOwner = owner
	trayMenuHover = -1

	itemCount := int32(len(trayMenuItems))
	height := menuPad*2 + itemCount*menuItemH + (itemCount-1)*menuItemGap

	var pt point
	procGetCursorPos.Call(uintptr(unsafe.Pointer(&pt)))
	sw, _, _ := procGetSystemMetrics.Call(smCxscreen)
	sh, _, _ := procGetSystemMetrics.Call(smCyscreen)
	x := pt.X
	y := pt.Y
	if int32(x)+menuWidth > int32(sw) {
		x = int32(sw) - menuWidth - 4
	}
	if int32(y)+height > int32(sh) {
		y = int32(sh) - height - 4
	}

	hinst, _, _ := procGetModuleHandle.Call(0)
	className := windows.StringToUTF16Ptr("ACTrayMenu")
	hwnd, _, _ := procCreateWindowEx.Call(
		wsExTopmost|wsExToolWin,
		uintptr(unsafe.Pointer(className)),
		0,
		wsPopup,
		uintptr(x), uintptr(y),
		uintptr(menuWidth), uintptr(height),
		owner, 0, hinst, 0,
	)
	if hwnd == 0 {
		return
	}
	trayMenuHwnd = hwnd

	pref := int32(dwmwcpRound)
	procDwmSetWindowAttribute.Call(hwnd, dwmwaWindowCornerPreference, uintptr(unsafe.Pointer(&pref)), unsafe.Sizeof(pref))

	procAnimateWindow.Call(hwnd, openAnimMs, awBlend|awCenter)
	procSetForeground.Call(hwnd)
	procSetFocus.Call(hwnd)
}

func closeTrayMenu() {
	if trayMenuHwnd != 0 {
		h := trayMenuHwnd
		trayMenuHwnd = 0
		procAnimateWindow.Call(h, closeAnimMs, awHide|awBlend|awCenter)
		procDestroyWindow.Call(h)
	}
}

func trayMenuItemAt(y int32) int32 {
	rel := y - menuPad
	if rel < 0 {
		return -1
	}
	idx := rel / (menuItemH + menuItemGap)
	within := rel % (menuItemH + menuItemGap)
	if within >= menuItemH || idx >= int32(len(trayMenuItems)) {
		return -1
	}
	return idx
}

func trayMenuWndProc(hwnd, msg, wparam, lparam uintptr) uintptr {
	switch msg {
	case wmEraseBkgnd:
		return 1
	case wmPaint:
		paintTrayMenu(hwnd)
		return 0
	case wmMouseMove:
		y := int32(int16(uint16((lparam >> 16) & 0xffff)))
		idx := trayMenuItemAt(y)
		if idx != trayMenuHover {
			trayMenuHover = idx
			procInvalidateRect.Call(hwnd, 0, 0)
		}
		return 0
	case wmLButtonUp:
		y := int32(int16(uint16((lparam >> 16) & 0xffff)))
		idx := trayMenuItemAt(y)
		owner := trayMenuOwner
		closeTrayMenu()
		switch idx {
		case 0:
			restoreWindow(owner)
		case 1:
			quitApp(owner)
		}
		return 0
	case wmActivate:
		if uint16(wparam) == waInactive {
			closeTrayMenu()
		}
		return 0
	case wmDestroy:
		return 0
	}
	r, _, _ := procDefWindowProc.Call(hwnd, msg, wparam, lparam)
	return r
}

func paintTrayMenu(hwnd uintptr) {
	var ps paintStruct
	hdc, _, _ := procBeginPaint.Call(hwnd, uintptr(unsafe.Pointer(&ps)))
	defer procEndPaint.Call(hwnd, uintptr(unsafe.Pointer(&ps)))

	var rc winRect
	procGetClientRect.Call(hwnd, uintptr(unsafe.Pointer(&rc)))

	bgBrush, _, _ := procCreateSolidBrush.Call(menuBg)
	procFillRect.Call(hdc, uintptr(unsafe.Pointer(&rc)), bgBrush)
	procDeleteObject.Call(bgBrush)

	var lf logFont
	lf.lfHeight = -14
	lf.lfWeight = fwNormal
	lf.lfCharSet = defaultCharS
	face := windows.StringToUTF16(uiFontFace())
	copy(lf.lfFaceName[:], face)
	font, _, _ := procCreateFontIndirect.Call(uintptr(unsafe.Pointer(&lf)))
	oldFont, _, _ := procSelectObject.Call(hdc, font)

	procSetBkMode.Call(hdc, transparentBk)
	procSetTextColor.Call(hdc, menuText)

	nullPen, _, _ := procGetStockObject.Call(nullPenStock)
	for i, label := range trayMenuItems {
		top := int32(menuPad) + int32(i)*(menuItemH+menuItemGap)
		if int32(i) == trayMenuHover {
			hoverBrush, _, _ := procCreateSolidBrush.Call(menuHoverB)
			oldBrush, _, _ := procSelectObject.Call(hdc, hoverBrush)
			oldPen, _, _ := procSelectObject.Call(hdc, nullPen)
			procRoundRect.Call(hdc, uintptr(menuPad), uintptr(top), uintptr(int32(menuWidth)-menuPad), uintptr(top+menuItemH), menuItemR, menuItemR)
			procSelectObject.Call(hdc, oldBrush)
			procSelectObject.Call(hdc, oldPen)
			procDeleteObject.Call(hoverBrush)
		}
		textRc := winRect{Left: menuPad + 12, Top: top, Right: int32(menuWidth) - menuPad - 6, Bottom: top + menuItemH}
		textPtr := windows.StringToUTF16Ptr(label)
		procDrawText.Call(hdc, uintptr(unsafe.Pointer(textPtr)), ^uintptr(0), uintptr(unsafe.Pointer(&textRc)), dtSingleLine|dtVCenter|dtLeft)
	}

	procSelectObject.Call(hdc, oldFont)
	procDeleteObject.Call(font)
}

func uiFontFace() string {
	return "Segoe UI"
}
