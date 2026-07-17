package launcher

import (
	"os/exec"
	"path/filepath"
	"syscall"
)

func detach(cmd *exec.Cmd) {
	cmd.SysProcAttr = &syscall.SysProcAttr{Setpgid: true}
}

func javaCandidates(cristalix string) []string {
	return []string{
		filepath.Join(cristalix, "runtime", "bin", "java"),
		"/usr/lib/jvm/default-java/bin/java",
		"/usr/lib/jvm/default/bin/java",
	}
}
