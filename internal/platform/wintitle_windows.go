package platform

import (
	"strings"
	"sync"
	"unsafe"

	"golang.org/x/sys/windows"
)

var procSendMessageTimeout = user32Win.NewProc("SendMessageTimeoutW")

const wmSetText = 0x000C
const smtoAbortIfHung = 0x0002

var titleMu sync.Mutex
var titleTargetPid uint32
var titleText string
var titleDone int
var titleCallback = windows.NewCallback(titleWindowProc)

func titleWindowProc(hwnd, lparam uintptr) uintptr {
	if vis, _, _ := procIsWindowVisible.Call(hwnd); vis == 0 {
		return 1
	}
	length, _, _ := procGetWindowTextLengthW.Call(hwnd)
	if length == 0 {
		return 1
	}
	var pid uint32
	procGetWindowThreadProcessId.Call(hwnd, uintptr(unsafe.Pointer(&pid)))
	if pid != titleTargetPid {
		return 1
	}
	buf := make([]uint16, length+1)
	procGetWindowTextW.Call(hwnd, uintptr(unsafe.Pointer(&buf[0])), length+1)
	if strings.TrimSpace(windows.UTF16ToString(buf)) == titleText {
		titleDone++
		return 1
	}
	ptr, err := windows.UTF16PtrFromString(titleText)
	if err != nil {
		return 1
	}
	var result uintptr
	procSendMessageTimeout.Call(hwnd, wmSetText, 0, uintptr(unsafe.Pointer(ptr)), smtoAbortIfHung, 800, uintptr(unsafe.Pointer(&result)))
	titleDone++
	return 1
}

func SetWindowTitleForPid(pid uint32, title string) bool {
	if pid == 0 || strings.TrimSpace(title) == "" {
		return false
	}
	titleMu.Lock()
	defer titleMu.Unlock()
	titleTargetPid = pid
	titleText = strings.TrimSpace(title)
	titleDone = 0
	procEnumWindows.Call(titleCallback, 0)
	return titleDone > 0
}
