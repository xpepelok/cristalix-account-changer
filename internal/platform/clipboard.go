package platform

import (
	"unsafe"

	"golang.org/x/sys/windows"
)

var procOpenClipboard = user32Win.NewProc("OpenClipboard")
var procEmptyClipboard = user32Win.NewProc("EmptyClipboard")
var procSetClipboardData = user32Win.NewProc("SetClipboardData")
var procCloseClipboard = user32Win.NewProc("CloseClipboard")
var procGlobalAlloc = kernel32Local.NewProc("GlobalAlloc")
var procGlobalLock = kernel32Local.NewProc("GlobalLock")
var procGlobalUnlock = kernel32Local.NewProc("GlobalUnlock")

const (
	gmemMoveable  = 0x0002
	cfUnicodeText = 13
)

func setClipboard(text string) bool {
	u, err := windows.UTF16FromString(text)
	if err != nil {
		return false
	}
	size := len(u) * 2

	hMem, _, _ := procGlobalAlloc.Call(gmemMoveable, uintptr(size))
	if hMem == 0 {
		return false
	}
	ptr, _, _ := procGlobalLock.Call(hMem)
	if ptr == 0 {
		return false
	}
	dst := unsafe.Slice((*uint16)(unsafe.Pointer(ptr)), len(u))
	copy(dst, u)
	procGlobalUnlock.Call(hMem)

	if r, _, _ := procOpenClipboard.Call(0); r == 0 {
		return false
	}
	procEmptyClipboard.Call()
	if r, _, _ := procSetClipboardData.Call(cfUnicodeText, hMem); r == 0 {
		procCloseClipboard.Call()
		return false
	}
	procCloseClipboard.Call()
	return true
}
