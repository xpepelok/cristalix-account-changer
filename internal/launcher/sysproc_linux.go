package launcher

import (
	"os"
	"os/exec"
	"path/filepath"
	"sort"
	"syscall"
)

func detach(cmd *exec.Cmd) {
	cmd.SysProcAttr = &syscall.SysProcAttr{Setpgid: true}
}

func hideConsole(cmd *exec.Cmd) {
	cmd.SysProcAttr = &syscall.SysProcAttr{Setpgid: true}
}

func makeDirLink(link, target string) error {
	return os.Symlink(target, link)
}

func javaCandidates(cristalix string) []string {
	var out []string
	if m, err := filepath.Glob(filepath.Join(cristalix, "updates", "*jre*", "bin", "java")); err == nil {
		sort.Strings(m)
		out = append(out, m...)
	}
	out = append(out, filepath.Join(cristalix, "runtime", "bin", "java"))
	if jh := os.Getenv("JAVA_HOME"); jh != "" {
		out = append(out, filepath.Join(jh, "bin", "java"))
	}
	if p, err := exec.LookPath("java"); err == nil {
		out = append(out, p)
	}
	out = append(out,
		"/usr/lib/jvm/default-java/bin/java",
		"/usr/lib/jvm/default/bin/java",
	)
	return out
}
