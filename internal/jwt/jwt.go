package jwt

import (
	"encoding/base64"
	"encoding/json"
	"errors"
	"strings"
)

type Claims struct {
	UUID    string `json:"uuid"`
	Purpose string `json:"purpose"`
	Iat     int64  `json:"iat"`
	Exp     int64  `json:"exp"`
}

func Parse(token string) (Claims, error) {
	var claims Claims
	parts := strings.Split(token, ".")
	if len(parts) < 2 {
		return claims, errors.New("malformed token")
	}
	payload, err := base64.RawURLEncoding.DecodeString(parts[1])
	if err != nil {
		payload, err = base64.StdEncoding.DecodeString(parts[1])
		if err != nil {
			return claims, err
		}
	}
	if err := json.Unmarshal(payload, &claims); err != nil {
		return claims, err
	}
	if claims.UUID == "" {
		return claims, errors.New("token has no uuid")
	}
	return claims, nil
}
