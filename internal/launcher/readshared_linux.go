package launcher

import "os"

func readFileShareAll(path string) ([]byte, error) {
	return os.ReadFile(path)
}
