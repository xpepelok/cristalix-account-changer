package platform

import (
	"sync"
	"unsafe"

	"golang.org/x/sys/windows"
)

var procEnumWindows = user32Win.NewProc("EnumWindows")
var procGetWindowTextW = user32Win.NewProc("GetWindowTextW")
var procGetWindowTextLengthW = user32Win.NewProc("GetWindowTextLengthW")
var procGetWindowThreadProcessId = user32Win.NewProc("GetWindowThreadProcessId")
var procIsWindowVisible = user32Win.NewProc("IsWindowVisible")

var enumMu sync.Mutex
var enumResult map[uint32][]string
var enumCallback = windows.NewCallback(enumWindowProc)

func enumWindowProc(hwnd, lparam uintptr) uintptr {
	if vis, _, _ := procIsWindowVisible.Call(hwnd); vis == 0 {
		return 1
	}
	length, _, _ := procGetWindowTextLengthW.Call(hwnd)
	if length == 0 {
		return 1
	}
	buf := make([]uint16, length+1)
	procGetWindowTextW.Call(hwnd, uintptr(unsafe.Pointer(&buf[0])), length+1)
	title := windows.UTF16ToString(buf)
	if title == "" {
		return 1
	}
	var pid uint32
	procGetWindowThreadProcessId.Call(hwnd, uintptr(unsafe.Pointer(&pid)))
	enumResult[pid] = append(enumResult[pid], title)
	return 1
}

func WindowTitlesByPID() map[uint32][]string {
	enumMu.Lock()
	defer enumMu.Unlock()
	enumResult = map[uint32][]string{}
	procEnumWindows.Call(enumCallback, 0)
	return enumResult
}

var closeMu sync.Mutex
var closeTargetPid uint32
var closeSent int
var closeCallback = windows.NewCallback(closeWindowProc)

func closeWindowProc(hwnd, lparam uintptr) uintptr {
	if vis, _, _ := procIsWindowVisible.Call(hwnd); vis == 0 {
		return 1
	}
	if length, _, _ := procGetWindowTextLengthW.Call(hwnd); length == 0 {
		return 1
	}
	var pid uint32
	procGetWindowThreadProcessId.Call(hwnd, uintptr(unsafe.Pointer(&pid)))
	if pid == closeTargetPid {
		procPostMessage.Call(hwnd, wmClose, 0, 0)
		closeSent++
	}
	return 1
}

var findPidMu sync.Mutex
var findPidTarget uint32
var findPidResult uintptr
var findPidCallback = windows.NewCallback(findPidWindowProc)

func findPidWindowProc(hwnd, lparam uintptr) uintptr {
	if findPidResult != 0 {
		return 0
	}
	if vis, _, _ := procIsWindowVisible.Call(hwnd); vis == 0 {
		return 1
	}
	if length, _, _ := procGetWindowTextLengthW.Call(hwnd); length == 0 {
		return 1
	}
	var pid uint32
	procGetWindowThreadProcessId.Call(hwnd, uintptr(unsafe.Pointer(&pid)))
	if pid == findPidTarget {
		findPidResult = hwnd
		return 0
	}
	return 1
}

func WindowForPid(pid uint32) uintptr {
	findPidMu.Lock()
	defer findPidMu.Unlock()
	findPidTarget = pid
	findPidResult = 0
	procEnumWindows.Call(findPidCallback, 0)
	return findPidResult
}

func CloseGame(pid uint32) bool {
	if pid == 0 {
		return false
	}
	closeMu.Lock()
	defer closeMu.Unlock()
	closeTargetPid = pid
	closeSent = 0
	procEnumWindows.Call(closeCallback, 0)
	if closeSent > 0 {
		return true
	}
	if handle, err := windows.OpenProcess(windows.PROCESS_TERMINATE, false, pid); err == nil {
		defer windows.CloseHandle(handle)
		return windows.TerminateProcess(handle, 1) == nil
	}
	return false
}
