package platform

import (
	"fmt"
	"syscall"
	"unsafe"
)

var crypt32 = syscall.NewLazyDLL("crypt32.dll")
var kernel32Local = syscall.NewLazyDLL("kernel32.dll")
var procProtectData = crypt32.NewProc("CryptProtectData")
var procUnprotect = crypt32.NewProc("CryptUnprotectData")
var procLocalFreeMem = kernel32Local.NewProc("LocalFree")

type dataBlob struct {
	size uint32
	data *byte
}

func newBlob(b []byte) dataBlob {
	if len(b) == 0 {
		return dataBlob{}
	}
	return dataBlob{size: uint32(len(b)), data: &b[0]}
}

func (b dataBlob) bytes() []byte {
	out := make([]byte, b.size)
	copy(out, unsafe.Slice(b.data, b.size))
	return out
}

const cryptUIForbidden = 0x1

func DPAPIEncrypt(plain []byte) ([]byte, error) {
	in := newBlob(plain)
	var out dataBlob
	ret, _, err := procProtectData.Call(
		uintptr(unsafe.Pointer(&in)),
		0, 0, 0, 0,
		cryptUIForbidden,
		uintptr(unsafe.Pointer(&out)),
	)
	if ret == 0 {
		return nil, fmt.Errorf("protect data: %w", err)
	}
	defer procLocalFreeMem.Call(uintptr(unsafe.Pointer(out.data)))
	return out.bytes(), nil
}

func DPAPIDecrypt(enc []byte) ([]byte, error) {
	in := newBlob(enc)
	var out dataBlob
	ret, _, err := procUnprotect.Call(
		uintptr(unsafe.Pointer(&in)),
		0, 0, 0, 0,
		cryptUIForbidden,
		uintptr(unsafe.Pointer(&out)),
	)
	if ret == 0 {
		return nil, fmt.Errorf("unprotect data: %w", err)
	}
	defer procLocalFreeMem.Call(uintptr(unsafe.Pointer(out.data)))
	return out.bytes(), nil
}
