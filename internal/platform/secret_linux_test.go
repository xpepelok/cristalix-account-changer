package platform

import (
	"os"
	"testing"
)

func TestKeyFileIsOwnerOnly(t *testing.T) {
	isolateSecrets(t)

	if _, err := Encrypt([]byte("x")); err != nil {
		t.Fatalf("Encrypt: %v", err)
	}
	info, err := os.Stat(keyPath())
	if err != nil {
		t.Fatalf("stat key: %v", err)
	}

	if perm := info.Mode().Perm(); perm != 0o600 {
		t.Fatalf("key mode = %04o, want 0600", perm)
	}
	if info.Size() != keyLen {
		t.Fatalf("key size = %d, want %d", info.Size(), keyLen)
	}
}

func TestKeyIsReusedAcrossRestarts(t *testing.T) {
	isolateSecrets(t)

	enc, err := Encrypt([]byte("token"))
	if err != nil {
		t.Fatalf("Encrypt: %v", err)
	}

	resetSecretCache()

	got, err := Decrypt(enc)
	if err != nil {
		t.Fatalf("Decrypt after restart: %v", err)
	}
	if string(got) != "token" {
		t.Fatalf("got %q, want %q", got, "token")
	}
}

func TestCorruptKeyIsNotSilentlyReplaced(t *testing.T) {
	isolateSecrets(t)

	if _, err := Encrypt([]byte("x")); err != nil {
		t.Fatalf("Encrypt: %v", err)
	}
	path := keyPath()
	if err := os.WriteFile(path, []byte("short"), 0o600); err != nil {
		t.Fatalf("write short key: %v", err)
	}
	resetSecretCache()

	if _, err := Encrypt([]byte("x")); err == nil {
		t.Fatal("Encrypt silently accepted a corrupt key file")
	}
	raw, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read key: %v", err)
	}
	if string(raw) != "short" {
		t.Fatal("corrupt key file was overwritten instead of reported")
	}
}
