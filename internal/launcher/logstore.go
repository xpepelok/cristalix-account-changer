package launcher

import (
	"bufio"
	"encoding/json"
	"fmt"
	"io"
	"os"
	"sync"
	"time"
)

const maxLogLines = 4000
const logPersistInterval = 3 * time.Second

type LogLine struct {
	At   int64  `json:"at"`
	Text string `json:"text"`
}
type logSession struct {
	ID      string
	Started int64
	Ended   int64
	Active  bool
	Lines   []LogLine
}
type logSessionInfo struct {
	ID      string `json:"id"`
	Started int64  `json:"started"`
	Ended   int64  `json:"ended"`
	Active  bool   `json:"active"`
	Lines   int    `json:"lines"`
}
type clientLog struct {
	uuid     string
	name     string
	sessions []*logSession
}
type LogStore struct {
	mu       sync.Mutex
	logs     map[string]*clientLog
	path     string
	lastSave time.Time
}

type persistedSession struct {
	ID      string    `json:"id"`
	Started int64     `json:"started"`
	Ended   int64     `json:"ended"`
	Active  bool      `json:"active"`
	Lines   []LogLine `json:"lines"`
}
type persistedClientLog struct {
	Name     string             `json:"name"`
	Sessions []persistedSession `json:"sessions"`
}

func NewLogStore(path string) *LogStore {
	s := &LogStore{logs: map[string]*clientLog{}, path: path}
	s.load()
	return s
}

func (s *LogStore) load() {
	if s.path == "" {
		return
	}
	data, err := os.ReadFile(s.path)
	if err != nil {
		return
	}
	var snapshot map[string]persistedClientLog
	if json.Unmarshal(data, &snapshot) != nil {
		return
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	for uuid, pcl := range snapshot {
		cl := &clientLog{uuid: uuid, name: pcl.Name}
		for _, ps := range pcl.Sessions {
			sess := &logSession{ID: ps.ID, Started: ps.Started, Ended: ps.Ended, Active: ps.Active, Lines: ps.Lines}
			if sess.Active {
				sess.Active = false
				sess.Ended = lastLineAt(sess)
			}
			cl.sessions = append(cl.sessions, sess)
		}
		s.logs[uuid] = cl
	}
}

func (s *LogStore) persist() {
	if s.path == "" {
		return
	}
	s.mu.Lock()
	snapshot := make(map[string]persistedClientLog, len(s.logs))
	for uuid, cl := range s.logs {
		sessions := make([]persistedSession, len(cl.sessions))
		for i, x := range cl.sessions {
			sessions[i] = persistedSession{ID: x.ID, Started: x.Started, Ended: x.Ended, Active: x.Active, Lines: append([]LogLine(nil), x.Lines...)}
		}
		snapshot[uuid] = persistedClientLog{Name: cl.name, Sessions: sessions}
	}
	s.lastSave = time.Now()
	s.mu.Unlock()

	data, err := json.Marshal(snapshot)
	if err != nil {
		return
	}
	tmp := s.path + ".tmp"
	if os.WriteFile(tmp, data, 0o644) == nil {
		_ = os.Rename(tmp, s.path)
	}
}

func (s *LogStore) Flush() { s.persist() }

func (s *LogStore) begin(uuid, name string) {
	s.mu.Lock()
	cl := s.logs[uuid]
	if cl == nil {
		cl = &clientLog{uuid: uuid}
		s.logs[uuid] = cl
	}
	if previous := s.current(cl); previous != nil && previous.Active {
		previous.Active = false
		previous.Ended = lastLineAt(previous)
	}
	cl.name = name
	now := time.Now().Unix()
	cl.sessions = append(cl.sessions, &logSession{ID: fmt.Sprintf("%d", time.Now().UnixNano()), Started: now, Active: true})
	s.mu.Unlock()
	s.persist()
}
func (s *LogStore) unsupported(uuid, name, msg string) {
	s.begin(uuid, name)
	s.append(uuid, msg)
	s.finish(uuid)
}
func (s *LogStore) current(cl *clientLog) *logSession {
	if cl == nil || len(cl.sessions) == 0 {
		return nil
	}
	return cl.sessions[len(cl.sessions)-1]
}
func (s *LogStore) append(uuid, text string) {
	s.mu.Lock()
	session := s.current(s.logs[uuid])
	if session == nil {
		s.mu.Unlock()
		return
	}
	session.Lines = append(session.Lines, LogLine{At: time.Now().Unix(), Text: text})
	if len(session.Lines) > maxLogLines {
		session.Lines = session.Lines[len(session.Lines)-maxLogLines:]
	}
	shouldSave := time.Since(s.lastSave) > logPersistInterval
	s.mu.Unlock()
	if shouldSave {
		s.persist()
	}
}
func (s *LogStore) finish(uuid string) {
	s.mu.Lock()
	if session := s.current(s.logs[uuid]); session != nil && session.Active {
		session.Active = false
		session.Ended = lastLineAt(session)
	}
	s.mu.Unlock()
	s.persist()
}

func lastLineAt(session *logSession) int64 {
	if len(session.Lines) > 0 {
		return session.Lines[len(session.Lines)-1].At
	}
	return time.Now().Unix()
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
		cur := s.current(cl)
		if cur != nil {
			out = append(out, logSummary{cl.uuid, cl.name, len(cur.Lines), cur.Active})
		}
	}
	return out
}
func (s *LogStore) Sessions(uuid string) []logSessionInfo {
	s.mu.Lock()
	defer s.mu.Unlock()
	cl := s.logs[uuid]
	if cl == nil {
		return nil
	}
	out := make([]logSessionInfo, 0, len(cl.sessions))
	for i := len(cl.sessions) - 1; i >= 0; i-- {
		x := cl.sessions[i]
		out = append(out, logSessionInfo{x.ID, x.Started, x.Ended, x.Active, len(x.Lines)})
	}
	return out
}
func (s *LogStore) Get(uuid, id string) ([]LogLine, bool) {
	s.mu.Lock()
	defer s.mu.Unlock()
	cl := s.logs[uuid]
	if cl == nil {
		return nil, false
	}
	session := s.current(cl)
	if id != "" {
		for _, x := range cl.sessions {
			if x.ID == id {
				session = x
				break
			}
		}
	}
	if session == nil {
		return nil, false
	}
	out := append([]LogLine(nil), session.Lines...)
	return out, true
}
func (s *LogStore) Clear(uuid string) {
	s.mu.Lock()
	delete(s.logs, uuid)
	s.mu.Unlock()
	s.persist()
}
func (s *LogStore) ClearAll() {
	s.mu.Lock()
	for uuid, cl := range s.logs {
		out := cl.sessions[:0]
		for _, x := range cl.sessions {
			if x.Active {
				out = append(out, x)
			}
		}
		cl.sessions = out
		if len(cl.sessions) == 0 {
			delete(s.logs, uuid)
		}
	}
	s.mu.Unlock()
	s.persist()
}
func (s *LogStore) DeleteSession(uuid, id string) {
	s.mu.Lock()
	cl := s.logs[uuid]
	if cl == nil {
		s.mu.Unlock()
		return
	}
	out := cl.sessions[:0]
	for _, x := range cl.sessions {
		if x.ID != id || x.Active {
			out = append(out, x)
		}
	}
	cl.sessions = out
	if len(cl.sessions) == 0 {
		delete(s.logs, uuid)
	}
	s.mu.Unlock()
	s.persist()
}
func (s *LogStore) Size(uuid string) int {
	s.mu.Lock()
	defer s.mu.Unlock()
	cl := s.logs[uuid]
	if cl == nil {
		return 0
	}
	n := 0
	for _, x := range cl.sessions {
		for _, line := range x.Lines {
			n += len(line.Text) + 1
		}
	}
	return n
}
func (s *LogStore) SizeAll() int {
	s.mu.Lock()
	defer s.mu.Unlock()
	n := 0
	for _, cl := range s.logs {
		for _, x := range cl.sessions {
			for _, line := range x.Lines {
				n += len(line.Text) + 1
			}
		}
	}
	return n
}

type LogStats struct {
	TotalAccounts     int    `json:"totalAccounts"`
	TotalSessions     int    `json:"totalSessions"`
	ActiveNow         int    `json:"activeNow"`
	TotalDuration     int64  `json:"totalDuration"`
	AvgDuration       int64  `json:"avgDuration"`
	LongestDuration   int64  `json:"longestDuration"`
	LongestName       string `json:"longestName"`
	LongestUUID       string `json:"longestUuid"`
	MostLaunchedName  string `json:"mostLaunchedName"`
	MostLaunchedUUID  string `json:"mostLaunchedUuid"`
	MostLaunchedCount int    `json:"mostLaunchedCount"`
	TotalLines        int    `json:"totalLines"`
}

func (s *LogStore) Stats() LogStats {
	s.mu.Lock()
	defer s.mu.Unlock()
	var st LogStats
	now := time.Now().Unix()
	st.TotalAccounts = len(s.logs)
	for _, cl := range s.logs {
		if len(cl.sessions) > st.MostLaunchedCount {
			st.MostLaunchedCount = len(cl.sessions)
			st.MostLaunchedName = cl.name
			st.MostLaunchedUUID = cl.uuid
		}
		for _, x := range cl.sessions {
			st.TotalSessions++
			end := x.Ended
			if x.Active {
				st.ActiveNow++
				end = now
			}
			dur := end - x.Started
			if dur < 0 {
				dur = 0
			}
			st.TotalDuration += dur
			if dur > st.LongestDuration {
				st.LongestDuration = dur
				st.LongestName = cl.name
				st.LongestUUID = cl.uuid
			}
			st.TotalLines += len(x.Lines)
		}
	}
	if st.TotalSessions > 0 {
		st.AvgDuration = st.TotalDuration / int64(st.TotalSessions)
	}
	return st
}

func (s *LogStore) pump(uuid string, r io.Reader) {
	sc := bufio.NewScanner(r)
	sc.Buffer(make([]byte, 0, 64*1024), 1024*1024)
	for sc.Scan() {
		s.append(uuid, sc.Text())
	}
}
