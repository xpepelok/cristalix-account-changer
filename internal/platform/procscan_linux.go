package platform

import (
	"syscall"
	"time"
)

const closeGrace = 10 * time.Second

func CloseGame(pid uint32) bool {
	if pid == 0 || !processAlive(pid) {
		return false
	}
	if syscall.Kill(int(pid), syscall.SIGTERM) != nil {
		return false
	}
	go func() {
		deadline := time.Now().Add(closeGrace)
		for time.Now().Before(deadline) {
			if !processAlive(pid) {
				return
			}
			time.Sleep(250 * time.Millisecond)
		}
		_ = syscall.Kill(int(pid), syscall.SIGKILL)
	}()
	return true
}

func processAlive(pid uint32) bool {
	return syscall.Kill(int(pid), 0) == nil
}
