package platform

import (
	"crypto/aes"
	"crypto/cipher"
	"crypto/rand"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"sync"
)

const keyLen = 32

var keyMu sync.Mutex
var cachedKey []byte

func keyPath() string {
	return filepath.Join(dataDir(), "key")
}

func resetSecretCache() {
	keyMu.Lock()
	cachedKey = nil
	keyMu.Unlock()
}

func secretKey() ([]byte, error) {
	keyMu.Lock()
	defer keyMu.Unlock()
	if cachedKey != nil {
		return cachedKey, nil
	}

	path := keyPath()
	raw, err := os.ReadFile(path)
	switch {
	case err == nil:
		if len(raw) != keyLen {
			return nil, fmt.Errorf("key file %s is corrupt (%d bytes, want %d)", path, len(raw), keyLen)
		}
		cachedKey = raw
		return cachedKey, nil
	case !errors.Is(err, os.ErrNotExist):
		return nil, err
	}

	if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
		return nil, err
	}
	fresh := make([]byte, keyLen)
	if _, err := rand.Read(fresh); err != nil {
		return nil, err
	}
	f, err := os.OpenFile(path, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0o600)
	if err != nil {
		if errors.Is(err, os.ErrExist) {
			raw, rerr := os.ReadFile(path)
			if rerr != nil {
				return nil, rerr
			}
			if len(raw) != keyLen {
				return nil, fmt.Errorf("key file %s is corrupt (%d bytes, want %d)", path, len(raw), keyLen)
			}
			cachedKey = raw
			return cachedKey, nil
		}
		return nil, err
	}
	if _, err := f.Write(fresh); err != nil {
		f.Close()
		os.Remove(path)
		return nil, err
	}
	if err := f.Close(); err != nil {
		os.Remove(path)
		return nil, err
	}
	cachedKey = fresh
	return cachedKey, nil
}

func aead() (cipher.AEAD, error) {
	key, err := secretKey()
	if err != nil {
		return nil, err
	}
	block, err := aes.NewCipher(key)
	if err != nil {
		return nil, err
	}
	return cipher.NewGCM(block)
}

func Encrypt(plain []byte) ([]byte, error) {
	gcm, err := aead()
	if err != nil {
		return nil, err
	}
	nonce := make([]byte, gcm.NonceSize())
	if _, err := rand.Read(nonce); err != nil {
		return nil, err
	}
	return gcm.Seal(nonce, nonce, plain, nil), nil
}

func Decrypt(enc []byte) ([]byte, error) {
	gcm, err := aead()
	if err != nil {
		return nil, err
	}
	if len(enc) < gcm.NonceSize() {
		return nil, errors.New("ciphertext too short")
	}
	nonce, body := enc[:gcm.NonceSize()], enc[gcm.NonceSize():]
	return gcm.Open(nil, nonce, body, nil)
}
