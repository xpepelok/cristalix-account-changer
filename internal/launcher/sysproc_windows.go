package launcher

import (
	"os"
	"os/exec"
	"path/filepath"
	"sort"
	"syscall"
)

const CreateNoWindow = 0x08000000

func detach(cmd *exec.Cmd) {
	cmd.SysProcAttr = &syscall.SysProcAttr{CreationFlags: CreateNoWindow}
}

func makeDirLink(link, target string) error {
	cmd := exec.Command("cmd", "/c", "mklink", "/J", link, target)
	cmd.SysProcAttr = &syscall.SysProcAttr{CreationFlags: CreateNoWindow}
	return cmd.Run()
}

func javaCandidates(cristalix string) []string {
	var out []string
	if m, err := filepath.Glob(filepath.Join(cristalix, "updates", "*jre*", "bin", "java.exe")); err == nil {
		sort.Strings(m)
		out = append(out, m...)
	}
	if jh := os.Getenv("JAVA_HOME"); jh != "" {
		out = append(out, filepath.Join(jh, "bin", "java.exe"))
	}
	if p, err := exec.LookPath("java"); err == nil {
		out = append(out, p)
	}
	out = append(out, filepath.Join(cristalix, "runtime", "bin", "java.exe"))
	return out
}
