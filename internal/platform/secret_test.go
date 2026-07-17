package platform

import (
	"bytes"
	"testing"
)

func isolateSecrets(t *testing.T) {
	t.Helper()
	t.Setenv("XDG_DATA_HOME", t.TempDir())
	resetSecretCache()
}

func TestEncryptRoundTrip(t *testing.T) {
	isolateSecrets(t)

	plain := []byte(`{"accounts":{"uuid":{"token":"jwt.value.here"}}}`)
	enc, err := Encrypt(plain)
	if err != nil {
		t.Fatalf("Encrypt: %v", err)
	}
	if bytes.Contains(enc, plain) {
		t.Fatal("ciphertext contains the plaintext")
	}

	got, err := Decrypt(enc)
	if err != nil {
		t.Fatalf("Decrypt: %v", err)
	}
	if !bytes.Equal(got, plain) {
		t.Fatalf("round trip mismatch:\n got %q\nwant %q", got, plain)
	}
}

func TestDecryptRejectsTamperedCiphertext(t *testing.T) {
	isolateSecrets(t)

	enc, err := Encrypt([]byte("secret token"))
	if err != nil {
		t.Fatalf("Encrypt: %v", err)
	}
	enc[len(enc)-1] ^= 0xff
	if _, err := Decrypt(enc); err == nil {
		t.Fatal("Decrypt accepted a tampered ciphertext")
	}
}

func TestDecryptRejectsGarbage(t *testing.T) {
	isolateSecrets(t)

	if _, err := Decrypt([]byte("not a ciphertext")); err == nil {
		t.Fatal("Decrypt accepted garbage")
	}
}
