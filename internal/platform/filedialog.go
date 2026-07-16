package platform

import (
	"runtime"
	"unicode/utf16"
	"unsafe"

	"golang.org/x/sys/windows"
)

var comdlg32 = windows.NewLazySystemDLL("comdlg32.dll")
var procGetOpenFileName = comdlg32.NewProc("GetOpenFileNameW")
var ole32Dlg = windows.NewLazySystemDLL("ole32.dll")
var procCoInitializeEx = ole32Dlg.NewProc("CoInitializeEx")
var procCoUninitialize = ole32Dlg.NewProc("CoUninitialize")

const coinitApartmentThreaded = 0x2

type openFileNameW struct {
	lStructSize       uint32
	hwndOwner         uintptr
	hInstance         uintptr
	lpstrFilter       *uint16
	lpstrCustomFilter *uint16
	nMaxCustFilter    uint32
	nFilterIndex      uint32
	lpstrFile         *uint16
	nMaxFile          uint32
	lpstrFileTitle    *uint16
	nMaxFileTitle     uint32
	lpstrInitialDir   *uint16
	lpstrTitle        *uint16
	flags             uint32
	nFileOffset       uint16
	nFileExtension    uint16
	lpstrDefExt       *uint16
	lCustData         uintptr
	lpfnHook          uintptr
	lpTemplateName    *uint16
	pvReserved        uintptr
	dwReserved        uint32
	flagsEx           uint32
}

const (
	ofnFileMustExist = 0x00001000
	ofnPathMustExist = 0x00000800
	ofnNoChangeDir   = 0x00000008
	ofnExplorer      = 0x00080000
	ofnHideReadOnly  = 0x00000004
)

func PickExecutable() string {
	result := make(chan string, 1)
	go func() {
		runtime.LockOSThread()
		defer runtime.UnlockOSThread()
		hr, _, _ := procCoInitializeEx.Call(0, coinitApartmentThreaded)
		if hr == 0 {
			defer procCoUninitialize.Call()
		}
		result <- pickExecutableDialog()
	}()
	return <-result
}

func pickExecutableDialog() string {
	buf := make([]uint16, 1024)
	filter := utf16.Encode([]rune("Лаунчер (*.exe)\x00*.exe\x00Все файлы\x00*.*\x00\x00"))
	title := utf16.Encode([]rune("Выберите файл лаунчера\x00"))

	var ofn openFileNameW
	ofn.lStructSize = uint32(unsafe.Sizeof(ofn))
	ofn.lpstrFilter = &filter[0]
	ofn.lpstrFile = &buf[0]
	ofn.nMaxFile = uint32(len(buf))
	ofn.lpstrTitle = &title[0]
	ofn.flags = ofnFileMustExist | ofnPathMustExist | ofnNoChangeDir | ofnExplorer | ofnHideReadOnly

	r, _, _ := procGetOpenFileName.Call(uintptr(unsafe.Pointer(&ofn)))
	if r == 0 {
		return ""
	}
	return windows.UTF16ToString(buf)
}
