package platform

import (
	"time"
	"unsafe"
)

var procSendInput = user32Win.NewProc("SendInput")
var procBringWindowToTop = user32Win.NewProc("BringWindowToTop")
var procAttachThreadInput = user32Win.NewProc("AttachThreadInput")
var procGetForegroundWindow = user32Win.NewProc("GetForegroundWindow")

const (
	inputKeyboard    = 1
	keyeventfKeyUp   = 0x0002
	keyeventfUnicode = 0x0004
	vkTab            = 0x09
	vkReturn         = 0x0D
)

type kbdInput struct {
	wVk         uint16
	wScan       uint16
	dwFlags     uint32
	time        uint32
	dwExtraInfo uintptr
}

type keyInput struct {
	inputType uint32
	_         uint32
	ki        kbdInput
	_         [8]byte
}

func sendKey(in keyInput) {
	procSendInput.Call(1, uintptr(unsafe.Pointer(&in)), unsafe.Sizeof(in))
}

func typeRune(r rune) {
	if r > 0xFFFF {
		return
	}
	down := keyInput{inputType: inputKeyboard}
	down.ki = kbdInput{wScan: uint16(r), dwFlags: keyeventfUnicode}
	sendKey(down)
	up := keyInput{inputType: inputKeyboard}
	up.ki = kbdInput{wScan: uint16(r), dwFlags: keyeventfUnicode | keyeventfKeyUp}
	sendKey(up)
}

func typeText(s string) {
	for _, r := range s {
		typeRune(r)
		time.Sleep(14 * time.Millisecond)
	}
}

func pressVK(vk uint16) {
	down := keyInput{inputType: inputKeyboard}
	down.ki = kbdInput{wVk: vk}
	sendKey(down)
	up := keyInput{inputType: inputKeyboard}
	up.ki = kbdInput{wVk: vk, dwFlags: keyeventfKeyUp}
	sendKey(up)
}

func focusWindow(hwnd uintptr) {
	procShowWindow.Call(hwnd, swRestore)
	procBringWindowToTop.Call(hwnd)
	fg, _, _ := procGetForegroundWindow.Call()
	if fg == hwnd {
		procSetForeground.Call(hwnd)
		return
	}
	var targetPid uint32
	targetThread, _, _ := procGetWindowThreadProcessId.Call(hwnd, uintptr(unsafe.Pointer(&targetPid)))
	var fgPid uint32
	fgThread, _, _ := procGetWindowThreadProcessId.Call(fg, uintptr(unsafe.Pointer(&fgPid)))
	if fgThread != 0 && fgThread != targetThread {
		procAttachThreadInput.Call(fgThread, targetThread, 1)
		procSetForeground.Call(hwnd)
		procAttachThreadInput.Call(fgThread, targetThread, 0)
	} else {
		procSetForeground.Call(hwnd)
	}
}
