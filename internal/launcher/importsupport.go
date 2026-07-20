package launcher

import (
	"os"
)

func LinkSharedDir(dst, src string) {
	if fi, err := os.Lstat(dst); err == nil {
		if fi.Mode()&(os.ModeSymlink|os.ModeIrregular) != 0 {
			return
		}
		_ = os.RemoveAll(dst)
	}
	if _, err := os.Stat(src); err != nil {
		return
	}
	_ = makeDirLink(dst, src)
}
