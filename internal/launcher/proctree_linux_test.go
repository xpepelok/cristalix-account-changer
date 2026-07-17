package launcher

import (
	"os"
	"testing"
)

func getpid() int { return os.Getpid() }

func TestIsGame(t *testing.T) {
	updates := "/home/u/.cristalix/updates/Minigames-java21"
	cases := []struct {
		name    string
		cmdline string
		want    bool
	}{
		{
			name:    "game client",
			cmdline: "/usr/bin/java -Xmx4G -cp " + updates + "/minecraft.jar:" + updates + "/libraries/guava.jar net.minecraft.client.main.Main --username Steve",
			want:    true,
		},
		{
			name:    "game client under a relocated updates dir",
			cmdline: "/usr/bin/java -cp /mnt/games/cristalix/Minigames/minecraft.jar ru.cristalix.client.Main",
			want:    true,
		},
		{
			name:    "jar launcher",
			cmdline: "/usr/bin/java -jar /home/u/.local/share/AccountChanger/Cristalix.jar",
			want:    false,
		},
		{
			name:    "staff launcher",
			cmdline: "/usr/bin/java -jar /home/u/.local/share/AccountChanger/CristalixLauncher.jar",
			want:    false,
		},
		{
			name:    "unrelated java process",
			cmdline: "/usr/bin/java -jar /opt/jetbrains/idea.jar",
			want:    false,
		},
		{
			name:    "empty",
			cmdline: "",
			want:    false,
		},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			p := procInfo{comm: "java", cmdline: tc.cmdline}
			if got := p.isGame(); got != tc.want {
				t.Fatalf("isGame() = %v, want %v\ncmdline: %s", got, tc.want, tc.cmdline)
			}
		})
	}
}

func TestIsJava(t *testing.T) {
	cases := []struct {
		name string
		p    procInfo
		want bool
	}{
		{"comm reports java", procInfo{comm: "java", cmdline: ""}, true},
		{"bundled runtime via cmdline", procInfo{comm: "", cmdline: "/home/u/.cristalix/runtime/bin/java -jar x.jar"}, true},

		{"truncated comm", procInfo{comm: "java", cmdline: "/very/long/path/to/jdk-21.0.1+12/bin/java -version"}, true},
		{"not java", procInfo{comm: "bash", cmdline: "/bin/bash -c echo"}, false},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			if got := tc.p.isJava(); got != tc.want {
				t.Fatalf("isJava() = %v, want %v", got, tc.want)
			}
		})
	}
}

func TestScanProcsFindsSelf(t *testing.T) {
	procs := scanProcs(true)
	if len(procs) == 0 {
		t.Fatal("scanProcs returned nothing")
	}
	var self *procInfo
	for i := range procs {
		if int(procs[i].pid) == getpid() {
			self = &procs[i]
			break
		}
	}
	if self == nil {
		t.Fatal("scanProcs did not report the test process itself")
	}
	if self.cmdline == "" {
		t.Fatal("own cmdline came back empty")
	}
	if self.ppid == 0 {
		t.Fatal("own ppid came back 0")
	}
}
