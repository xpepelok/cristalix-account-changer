package launcher

import (
	"bufio"
	"fmt"
	"io"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"syscall"
	"time"
)

const CreateNoWindow = 0x08000000

const launcherDownloadURL = "https://cristalix.gg/content/launcher/Cristalix.exe"
const JarLauncherURL = "https://cristalix.gg/content/launcher/Cristalix.jar"
const StaffLauncherURL = "https://cristalix.gg/content/launcher/new/CristalixLauncher.exe"

func EnsureLauncher(dest string) error {
	return EnsureLauncherFrom(dest, launcherDownloadURL)
}

func EnsureLauncherFrom(dest, downloadURL string) error {
	if info, err := os.Stat(dest); err == nil && info.Size() > 0 {
		return nil
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
	cmd.SysProcAttr = &syscall.SysProcAttr{CreationFlags: CreateNoWindow}
	return cmd.Start()
}

func ResolveJava(cristalix string) string {
	if p, err := exec.LookPath("java"); err == nil {
		return p
	}
	candidates := []string{
		filepath.Join(cristalix, "runtime", "bin", "java.exe"),
	}
	for _, c := range candidates {
		if _, err := os.Stat(c); err == nil {
			return c
		}
	}
	return "java"
}

func StartLauncherJar(java, launcher, uuid, name string, store *LogStore, onInvalid func(), track func(*os.Process)) error {
	if _, err := os.Stat(launcher); err != nil {
		return fmt.Errorf("launcher not found: %w", err)
	}
	cmd := exec.Command(java, "-jar", launcher)
	cmd.Dir = filepath.Dir(launcher)
	cmd.Env = CleanEnv()
	cmd.SysProcAttr = &syscall.SysProcAttr{CreationFlags: CreateNoWindow}
	if store == nil {
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
	store.begin(uuid, name)
	bad := make(chan struct{}, 1)
	go scanLauncher(store, uuid, stdout, bad)
	go scanLauncher(store, uuid, stderr, bad)
	go func() {
		select {
		case <-bad:
			_ = cmd.Process.Kill()
			store.unsupported(uuid, name, "Новый лаунчер не поддерживает чтение логов")
			if onInvalid != nil {
				onInvalid()
			}
		case <-time.After(6 * time.Second):
		}
		_ = cmd.Wait()
		if track != nil {
			track(nil)
		}
		store.finish(uuid)
	}()
	return nil
}

func scanLauncher(store *LogStore, uuid string, r io.Reader, bad chan struct{}) {
	sc := bufio.NewScanner(r)
	sc.Buffer(make([]byte, 0, 64*1024), 1024*1024)
	for sc.Scan() {
		line := sc.Text()
		store.append(uuid, line)
		low := strings.ToLower(line)
		if strings.Contains(low, "invalid or corrupt jarfile") || strings.Contains(low, "unable to access jarfile") {
			select {
			case bad <- struct{}{}:
			default:
			}
		}
	}
}

func CleanEnv() []string {
	return os.Environ()
}
