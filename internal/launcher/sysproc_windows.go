package launcher

import (
	"os/exec"
	"path/filepath"
	"syscall"
)

const CreateNoWindow = 0x08000000

func detach(cmd *exec.Cmd) {
	cmd.SysProcAttr = &syscall.SysProcAttr{CreationFlags: CreateNoWindow}
}

func javaCandidates(cristalix string) []string {
	return []string{filepath.Join(cristalix, "runtime", "bin", "java.exe")}
}
