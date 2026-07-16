package launcher

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"
)

type logCursor struct {
	offset  int64
	pending []string
}

func TailGameLog(updates, client, uuid, name string, store *LogStore, ready func(bool)) {
	if store == nil || updates == "" {
		if ready != nil {
			ready(false)
		}
		return
	}

	started := time.Now()
	deadline := started.Add(3 * time.Minute)
	cursors := map[string]*logCursor{}
	var claimed string

	store.begin(uuid, name)
	defer store.finish(uuid)
	store.append(uuid, "[AccountChanger] Ожидание live-лога клиента: "+client)

	for time.Now().Before(deadline) {
		if claimed != "" {
			cursor := cursors[claimed]
			for _, line := range readLogDelta(claimed, cursor) {
				store.append(uuid, line)
			}
			time.Sleep(120 * time.Millisecond)
			continue
		}

		paths, _ := filepath.Glob(filepath.Join(updates, "*", "logs", "latest.log"))
		for _, path := range paths {
			info, err := os.Stat(path)
			if err != nil || info.ModTime().Before(started.Add(-2*time.Second)) {
				continue
			}
			cursor := cursors[path]
			if cursor == nil {
				cursor = &logCursor{}
				cursors[path] = cursor
			}
			for _, line := range readLogDelta(path, cursor) {
				cursor.pending = append(cursor.pending, line)
				if len(cursor.pending) > 300 {
					cursor.pending = cursor.pending[len(cursor.pending)-300:]
				}
				if !launcherLogUserMatches(line, name) {
					continue
				}

				target := filepath.Join(filepath.Dir(path), uniqueLogName(name, uuid))
				if !renameOpenLog(path, target) {
					store.append(uuid, "[AccountChanger] Не удалось закрепить latest.log: файл занят лаунчером")
					if ready != nil {
						ready(false)
					}
					return
				}
				claimed = target
				cursors[claimed] = &logCursor{offset: cursor.offset}
				deadline = time.Now().Add(12 * time.Hour)
				store.append(uuid, "[AccountChanger] Live-лог закреплён: "+filepath.Base(target))
				for _, saved := range cursor.pending {
					store.append(uuid, saved)
				}
				if ready != nil {
					ready(true)
				}
				break
			}
			if claimed != "" {
				break
			}
		}
		time.Sleep(120 * time.Millisecond)
	}
	if claimed == "" && ready != nil {
		ready(false)
	}
}

func readLogDelta(path string, cursor *logCursor) []string {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil
	}
	if int64(len(data)) < cursor.offset {
		cursor.offset = 0
		cursor.pending = nil
	}
	chunk := string(data[cursor.offset:])
	cursor.offset = int64(len(data))
	if chunk == "" {
		return nil
	}
	lines := strings.Split(chunk, "\n")
	for i := range lines {
		lines[i] = strings.TrimSuffix(lines[i], "\r")
	}
	if !strings.HasSuffix(chunk, "\n") {
		cursor.offset -= int64(len(lines[len(lines)-1]))
		lines = lines[:len(lines)-1]
	}
	return lines
}

func uniqueLogName(name, uuid string) string {
	safe := strings.Map(func(r rune) rune {
		if r == '<' || r == '>' || r == ':' || r == '"' || r == '/' || r == '\\' || r == '|' || r == '?' || r == '*' {
			return '_'
		}
		return r
	}, strings.TrimSpace(name))
	short := uuid
	if len(short) > 8 {
		short = short[:8]
	}
	return fmt.Sprintf("%s-%s-%s.log", safe, time.Now().Format("2006-01-02_15-04-05"), short)
}

func renameOpenLog(from, to string) bool {
	deadline := time.Now().Add(5 * time.Second)
	for time.Now().Before(deadline) {
		if err := os.Rename(from, to); err == nil {
			return true
		}
		time.Sleep(80 * time.Millisecond)
	}
	return false
}
