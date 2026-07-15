package launcher

import (
	"bufio"
	"io"
	"sync"
	"time"
)

const maxLogLines = 4000

type LogLine struct {
	At   int64  `json:"at"`
	Text string `json:"text"`
}

type clientLog struct {
	uuid    string
	name    string
	started int64
	active  bool
	lines   []LogLine
}

type LogStore struct {
	mu   sync.Mutex
	logs map[string]*clientLog
}

func NewLogStore() *LogStore {
	return &LogStore{logs: map[string]*clientLog{}}
}

func (s *LogStore) begin(uuid, name string) {
	s.mu.Lock()
	defer s.mu.Unlock()
	cl, ok := s.logs[uuid]
	if !ok {
		cl = &clientLog{uuid: uuid, name: name}
		s.logs[uuid] = cl
	}
	cl.name = name
	cl.started = time.Now().Unix()
	cl.active = true
	cl.lines = append(cl.lines, LogLine{At: time.Now().Unix(), Text: "──── запуск ────"})
	s.trim(cl)
}

func (s *LogStore) unsupported(uuid, name, msg string) {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.logs[uuid] = &clientLog{
		uuid:    uuid,
		name:    name,
		started: time.Now().Unix(),
		active:  false,
		lines:   []LogLine{{At: time.Now().Unix(), Text: msg}},
	}
}

func (s *LogStore) append(uuid, text string) {
	s.mu.Lock()
	defer s.mu.Unlock()
	cl, ok := s.logs[uuid]
	if !ok {
		return
	}
	cl.lines = append(cl.lines, LogLine{At: time.Now().Unix(), Text: text})
	s.trim(cl)
}

func (s *LogStore) finish(uuid string) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if cl, ok := s.logs[uuid]; ok {
		cl.active = false
	}
}

func (s *LogStore) trim(cl *clientLog) {
	if len(cl.lines) > maxLogLines {
		cl.lines = cl.lines[len(cl.lines)-maxLogLines:]
	}
}

type logSummary struct {
	UUID   string `json:"uuid"`
	Name   string `json:"name"`
	Lines  int    `json:"lines"`
	Active bool   `json:"active"`
}

func (s *LogStore) Summaries() []logSummary {
	s.mu.Lock()
	defer s.mu.Unlock()
	out := make([]logSummary, 0, len(s.logs))
	for _, cl := range s.logs {
		out = append(out, logSummary{UUID: cl.uuid, Name: cl.name, Lines: len(cl.lines), Active: cl.active})
	}
	return out
}

func (s *LogStore) Get(uuid string) ([]LogLine, bool) {
	s.mu.Lock()
	defer s.mu.Unlock()
	cl, ok := s.logs[uuid]
	if !ok {
		return nil, false
	}
	out := make([]LogLine, len(cl.lines))
	copy(out, cl.lines)
	return out, true
}

func (s *LogStore) Clear(uuid string) {
	s.mu.Lock()
	defer s.mu.Unlock()
	delete(s.logs, uuid)
}

func (s *LogStore) pump(uuid string, r io.Reader) {
	scanner := bufio.NewScanner(r)
	scanner.Buffer(make([]byte, 0, 64*1024), 1024*1024)
	for scanner.Scan() {
		s.append(uuid, scanner.Text())
	}
}
