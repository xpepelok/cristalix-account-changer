package launcher

import (
	"errors"
	"path/filepath"
	"time"

	"golang.org/x/sys/unix"
)

func (w *Watcher) watchEvents() {
	dir := filepath.Dir(w.Paths.LauncherCfg)
	for {
		w.watchOnce(dir)
		time.Sleep(2 * time.Second)
	}
}

func (w *Watcher) watchOnce(dir string) {
	fd, err := unix.InotifyInit1(unix.IN_CLOEXEC)
	if err != nil {
		return
	}
	defer unix.Close(fd)

	const mask = unix.IN_CLOSE_WRITE | unix.IN_MOVED_TO | unix.IN_CREATE |
		unix.IN_MODIFY | unix.IN_DELETE_SELF | unix.IN_MOVE_SELF
	if _, err := unix.InotifyAddWatch(fd, dir, mask); err != nil {
		return
	}

	buf := make([]byte, 8192)
	for {
		n, err := unix.Read(fd, buf)
		if err != nil {
			if errors.Is(err, unix.EINTR) {
				continue
			}
			return
		}
		if n <= 0 {
			return
		}
		w.Capture()
	}
}
