package launcher

import (
	"archive/zip"
	"bufio"
	"fmt"
	"io"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"time"
)

const launcherDownloadURL = "https://cristalix.gg/content/launcher/Cristalix.exe"
const JarLauncherURL = "https://cristalix.gg/content/launcher/Cristalix.jar"
const StaffLauncherURL = "https://cristalix.gg/content/launcher/new/CristalixLauncher.exe"

func EnsureLauncher(dest string) error {
	return EnsureLauncherFrom(dest, launcherDownloadURL)
}

func jarIsValid(path string) bool {
	r, err := zip.OpenReader(path)
	if err != nil {
		return false
	}
	defer r.Close()
	for _, f := range r.File {
		if f.Name == "META-INF/MANIFEST.MF" {
			return true
		}
	}
	return false
}

func EnsureLauncherFrom(dest, downloadURL string) error {
	if info, err := os.Stat(dest); err == nil && info.Size() > 0 {
		if !strings.HasSuffix(dest, ".jar") || jarIsValid(dest) {
			return nil
		}
		_ = os.Remove(dest)
	}
	if err := os.MkdirAll(filepath.Dir(dest), 0o755); err != nil {
		return err
	}

	client := &http.Client{Timeout: 10 * time.Minute}
	resp, err := client.Get(downloadURL)
	if err != nil {
		return err
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("download launcher: status %d", resp.StatusCode)
	}

	tmp := dest + ".part"
	f, err := os.Create(tmp)
	if err != nil {
		return err
	}
	if _, err := io.Copy(f, resp.Body); err != nil {
		f.Close()
		os.Remove(tmp)
		return err
	}
	f.Close()
	return os.Rename(tmp, dest)
}

func StartLauncher(exe string) error {
	if _, err := os.Stat(exe); err != nil {
		return fmt.Errorf("launcher not found: %w", err)
	}
	cmd := exec.Command(exe)
	cmd.Dir = filepath.Dir(exe)
	cmd.Env = CleanEnv()
	detach(cmd)
	return cmd.Start()
}

func StartLauncherLogged(exe, home, uuid, name string, store *LogStore, track func(*os.Process), verified func(uint32)) error {
	if _, err := os.Stat(exe); err != nil {
		return fmt.Errorf("launcher not found: %w", err)
	}
	cmd := exec.Command(exe)
	cmd.Dir = filepath.Dir(exe)
	env := CleanEnv()
	if home != "" {
		env = withEnv(env, "USERPROFILE", home)
		env = withEnv(env, "APPDATA", home)
	}
	cmd.Env = env
	detach(cmd)
	stdout, err := cmd.StdoutPipe()
	if err != nil {
		return err
	}
	stderr, err := cmd.StderrPipe()
	if err != nil {
		return err
	}
	if err := cmd.Start(); err != nil {
		return err
	}
	if track != nil {
		track(cmd.Process)
	}
	if store != nil {
		store.begin(uuid, name)
	}
	matched := make(chan struct{}, 1)
	handle := func(line string) {
		if store != nil {
			store.append(uuid, line)
		}
		if launcherLogUserMatches(line, name) {
			select {
			case matched <- struct{}{}:
				if verified != nil {
					verified(uint32(cmd.Process.Pid))
				}
			default:
			}
		}
	}
	go scanLauncherLines(stdout, handle)
	go scanLauncherLines(stderr, handle)
	go func() {
		_ = cmd.Wait()
		if track != nil {
			track(nil)
		}
		if store != nil {
			store.finish(uuid)
		}
	}()
	return nil
}

func launcherLogUserMatches(line, expected string) bool {
	const marker = "setting user:"
	idx := strings.Index(strings.ToLower(line), marker)
	if idx < 0 {
		return false
	}
	user := strings.TrimSpace(line[idx+len(marker):])
	return user != "" && strings.EqualFold(user, strings.TrimSpace(expected))
}

func ResolveJava(cristalix string) string {

	java, _ := UsableJava(cristalix)
	return java
}

func UsableJava(cristalix string) (java string, ok bool) {
	for _, c := range javaCandidates(cristalix) {
		if _, err := os.Stat(c); err != nil {
			continue
		}
		if javaMajor(c) < 11 {
			continue
		}
		return c, true
	}
	return "java", false
}

func javaMajor(javaPath string) int {
	cmd := exec.Command(javaPath, "-version")
	hideConsole(cmd)
	out, err := cmd.CombinedOutput()
	if err != nil {
		return 0
	}
	return parseJavaMajor(string(out))
}

func parseJavaMajor(s string) int {
	i := strings.Index(s, "version \"")
	if i < 0 {
		return 0
	}
	v := s[i+len("version \""):]
	if j := strings.IndexByte(v, '"'); j >= 0 {
		v = v[:j]
	}
	v = strings.TrimPrefix(v, "1.")
	n := 0
	for k := 0; k < len(v) && v[k] >= '0' && v[k] <= '9'; k++ {
		n = n*10 + int(v[k]-'0')
	}
	return n
}

func StartLauncherJar(java, launcher, home string, onInvalid func(), track func(*os.Process)) error {
	if _, err := os.Stat(launcher); err != nil {
		return fmt.Errorf("launcher not found: %w", err)
	}
	args := []string{"-jar", launcher}
	env := CleanEnv()
	if home != "" {
		args = append([]string{"-Duser.home=" + home}, args...)
		env = withEnv(env, "APPDATA", home)
	}
	cmd := exec.Command(java, args...)
	cmd.Dir = filepath.Dir(launcher)
	cmd.Env = env
	detach(cmd)
	if onInvalid == nil && track == nil {
		return cmd.Start()
	}
	stdout, err := cmd.StdoutPipe()
	if err != nil {
		return err
	}
	stderr, err := cmd.StderrPipe()
	if err != nil {
		return err
	}
	if err := cmd.Start(); err != nil {
		return err
	}
	if track != nil {
		track(cmd.Process)
	}
	bad := make(chan struct{}, 1)
	go scanJarBroken(stdout, bad)
	go scanJarBroken(stderr, bad)
	go func() {
		select {
		case <-bad:
			_ = cmd.Process.Kill()
			if onInvalid != nil {
				onInvalid()
			}
		case <-time.After(6 * time.Second):
		}
		_ = cmd.Wait()
		if track != nil {
			track(nil)
		}
	}()
	return nil
}

func scanJarBroken(r io.Reader, bad chan struct{}) {
	sc := bufio.NewScanner(r)
	sc.Buffer(make([]byte, 0, 64*1024), 1024*1024)
	for sc.Scan() {
		low := strings.ToLower(sc.Text())
		if strings.Contains(low, "invalid or corrupt jarfile") || strings.Contains(low, "unable to access jarfile") {
			select {
			case bad <- struct{}{}:
			default:
			}
			return
		}
	}
}

func scanLauncherLines(r io.Reader, handle func(string)) {
	sc := bufio.NewScanner(r)
	sc.Buffer(make([]byte, 0, 64*1024), 1024*1024)
	for sc.Scan() {
		handle(sc.Text())
	}
}

func CleanEnv() []string {
	return os.Environ()
}

func withEnv(base []string, key, val string) []string {
	prefix := strings.ToUpper(key) + "="
	out := make([]string, 0, len(base)+1)
	for _, e := range base {
		if strings.HasPrefix(strings.ToUpper(e), prefix) {
			continue
		}
		out = append(out, e)
	}
	return append(out, key+"="+val)
}
