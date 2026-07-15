package platform

import "runtime/debug"

func TrimMemory() {
	debug.FreeOSMemory()
}
