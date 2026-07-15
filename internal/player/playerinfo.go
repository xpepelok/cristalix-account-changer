package player

import (
	"encoding/json"
	"io"
	"net/url"
	"strings"
	"sync"
	"time"

	fhttp "github.com/bogdanfinn/fhttp"
	tlsclient "github.com/bogdanfinn/tls-client"
	"github.com/bogdanfinn/tls-client/profiles"
)

type PlayerInfo struct {
	Name         string `json:"name"`
	Group        string `json:"group"`
	Staff        string `json:"staff"`
	Donate       string `json:"donate"`
	Color        string `json:"color"`
	NameColor    string `json:"nameColor"`
	Label        string `json:"label"`
	RegisteredAt string `json:"registeredAt"`
	Online       string `json:"online"`
	LastSeen     string `json:"lastSeen"`
	Subscription string `json:"subscription"`
	SubEnding    string `json:"subEnding"`
	Likes        int    `json:"likes"`
	Views        int    `json:"views"`
	Score        int    `json:"score"`
	DonateColor  string `json:"donateColor"`
}

type searchEntry struct {
	Name         string `json:"name"`
	RegisteredAt string `json:"registeredAt"`
	Groups       struct {
		Staff   string `json:"staff"`
		Donate  string `json:"donate"`
		Display string `json:"display"`
	} `json:"groups"`
	Social struct {
		Prefix         string `json:"prefix"`
		FormattedName  string `json:"formattedName"`
		LastSeenOnline string `json:"lastSeenOnline"`
	} `json:"social"`
	Status struct {
		Online        *bool `json:"online"`
		PrivacyHidden bool  `json:"privacyHidden"`
	} `json:"status"`
	Subscription struct {
		Key    string `json:"key"`
		Ending string `json:"ending"`
	} `json:"subscription"`
	Stats struct {
		Views int `json:"views"`
		Likes int `json:"likes"`
		Score int `json:"score"`
	} `json:"stats"`
}

type cachedInfo struct {
	info    *PlayerInfo
	fetched time.Time
}

var playerCacheMu sync.Mutex
var playerCache = map[string]cachedInfo{}
var playerClient tlsclient.HttpClient
var playerOnce sync.Once

const playerCacheTTL = 60 * time.Second
const playerNegativeTTL = 30 * time.Second
const chromeUA = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/133.0.0.0 Safari/537.36"

func initPlayerClient() {
	options := []tlsclient.HttpClientOption{
		tlsclient.WithTimeoutSeconds(8),
		tlsclient.WithClientProfile(profiles.Chrome_133),
		tlsclient.WithCookieJar(tlsclient.NewCookieJar()),
	}
	client, err := tlsclient.NewHttpClient(tlsclient.NewNoopLogger(), options...)
	if err == nil {
		playerClient = client
	}
}

func FetchPlayerInfo(name string) *PlayerInfo {
	key := strings.ToLower(name)

	playerCacheMu.Lock()
	if hit, ok := playerCache[key]; ok {
		ttl := playerCacheTTL
		if hit.info == nil {
			ttl = playerNegativeTTL
		}
		if time.Since(hit.fetched) < ttl {
			playerCacheMu.Unlock()
			return hit.info
		}
	}
	playerCacheMu.Unlock()

	info := fetchPlayerInfoRaw(name)

	playerCacheMu.Lock()
	playerCache[key] = cachedInfo{info: info, fetched: time.Now()}
	playerCacheMu.Unlock()

	return info
}

func fetchPlayerInfoRaw(name string) *PlayerInfo {
	playerOnce.Do(initPlayerClient)
	if playerClient == nil {
		return nil
	}
	for attempt := 0; attempt < 5; attempt++ {
		info, done := tryFetchPlayer(name)
		if done {
			return info
		}
		if attempt < 4 {
			time.Sleep(500 * time.Millisecond)
		}
	}
	return nil
}

func tryFetchPlayer(name string) (*PlayerInfo, bool) {
	endpoint := "https://top.cristalix.gg/api/web/players/search?q=" + url.QueryEscape(name) + "&limit=8"
	req, err := fhttp.NewRequest(fhttp.MethodGet, endpoint, nil)
	if err != nil {
		return nil, false
	}
	req.Header = fhttp.Header{
		"user-agent":         {chromeUA},
		"accept":             {"application/json, text/plain, */*"},
		"accept-language":    {"ru-RU,ru;q=0.9,en;q=0.8"},
		"referer":            {"https://top.cristalix.gg/"},
		"sec-ch-ua":          {`"Chromium";v="133", "Not(A:Brand";v="24", "Google Chrome";v="133"`},
		"sec-ch-ua-mobile":   {"?0"},
		"sec-ch-ua-platform": {`"Windows"`},
		"sec-fetch-dest":     {"empty"},
		"sec-fetch-mode":     {"cors"},
		"sec-fetch-site":     {"same-origin"},
	}

	resp, err := playerClient.Do(req)
	if err != nil {
		return nil, false
	}
	defer resp.Body.Close()
	if resp.StatusCode != 200 {
		return nil, false
	}
	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, false
	}

	var entries []searchEntry
	if json.Unmarshal(body, &entries) != nil {
		return nil, false
	}

	var match *searchEntry
	for i := range entries {
		if strings.EqualFold(entries[i].Name, name) {
			match = &entries[i]
			break
		}
	}
	if match == nil {
		return nil, true
	}

	color, label := parseFormatted(match.Social.Prefix)
	online := ""
	if !match.Status.PrivacyHidden && match.Status.Online != nil {
		if *match.Status.Online {
			online = "online"
		} else {
			online = "offline"
		}
	} else if match.Social.LastSeenOnline != "" {
		if t, err := time.Parse(time.RFC3339Nano, match.Social.LastSeenOnline); err == nil {
			if time.Since(t) < 5*time.Minute {
				online = "online"
			} else {
				online = "offline"
			}
		}
	}
	info := &PlayerInfo{
		Name:         match.Name,
		Group:        match.Groups.Display,
		Staff:        match.Groups.Staff,
		Donate:       match.Groups.Donate,
		Color:        color,
		NameColor:    colorOfName(match.Social.FormattedName, match.Name),
		Label:        label,
		RegisteredAt: match.RegisteredAt,
		Online:       online,
		LastSeen:     match.Social.LastSeenOnline,
		Subscription: match.Subscription.Key,
		SubEnding:    match.Subscription.Ending,
		Likes:        match.Stats.Likes,
		Views:        match.Stats.Views,
		Score:        match.Stats.Score,
		DonateColor:  donateColor(match.Groups.Donate),
	}

	return info, true
}

var legacyColors = map[rune]string{
	'0': "#4a4a4a", '1': "#3b5bdb", '2': "#2fb344", '3': "#22b8cf",
	'4': "#e03131", '5': "#ae3ec9", '6': "#f59f00", '7': "#adb5bd",
	'8': "#868e96", '9': "#4d7cff", 'a': "#51cf66", 'b': "#3bc9db",
	'c': "#ff6b6b", 'd': "#f06595", 'e': "#ffd43b", 'f': "#ffffff",
}

var donateColors = map[string]string{
	"IRON":         "#aaaaaa",
	"VIP":          "#ffff55",
	"VIP_PLUS":     "#ffff55",
	"GOLD":         "#ffff55",
	"PREMIUM":      "#55ff55",
	"PREMIUM_PLUS": "#55ff55",
	"DIAMOND":      "#55ffff",
	"MVP":          "#55ffff",
	"MVP_PLUS":     "#55ffff",
	"EMERALD":      "#00aa00",
	"SPONSOR":      "#ffaa00",
}

func donateColor(group string) string {
	norm := make([]rune, 0, len(group))
	for _, r := range strings.ToUpper(group) {
		if r >= 'A' && r <= 'Z' {
			norm = append(norm, r)
		}
	}
	return donateColors[string(norm)]
}

func parseFormatted(s string) (string, string) {
	runes := []rune(s)
	color := ""
	var out strings.Builder
	for i := 0; i < len(runes); i++ {
		c := runes[i]
		if c == '¨' && i+6 < len(runes) {
			hex := string(runes[i+1 : i+7])
			if isHex6(hex) {
				if color == "" {
					color = "#" + strings.ToLower(hex)
				}
				i += 6
				continue
			}
		}
		if c == '§' && i+1 < len(runes) {
			if color == "" {
				if hex, ok := legacyColors[toLowerRune(runes[i+1])]; ok {
					color = hex
				}
			}
			i++
			continue
		}
		out.WriteRune(c)
	}
	return color, strings.TrimSpace(out.String())
}

func toLowerRune(r rune) rune {
	if r >= 'A' && r <= 'Z' {
		return r + 32
	}
	return r
}

func colorOfName(formatted, name string) string {
	runes := []rune(formatted)
	target := []rune(name)
	color := ""
	for i := 0; i < len(runes); i++ {
		c := runes[i]
		if c == '¨' && i+6 < len(runes) && isHex6(string(runes[i+1:i+7])) {
			color = "#" + strings.ToLower(string(runes[i+1:i+7]))
			i += 6
			continue
		}
		if c == '§' && i+1 < len(runes) {
			i++
			continue
		}
		if startsWith(runes[i:], target) {
			return color
		}
	}
	return color
}

func startsWith(s, prefix []rune) bool {
	if len(s) < len(prefix) {
		return false
	}
	for i := range prefix {
		if s[i] != prefix[i] {
			return false
		}
	}
	return true
}

func isHex6(s string) bool {
	if len(s) != 6 {
		return false
	}
	for _, c := range s {
		ok := (c >= '0' && c <= '9') || (c >= 'a' && c <= 'f') || (c >= 'A' && c <= 'F')
		if !ok {
			return false
		}
	}
	return true
}
