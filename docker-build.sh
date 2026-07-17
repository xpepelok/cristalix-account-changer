#!/usr/bin/env bash
set -euo pipefail
export GOTOOLCHAIN=local CGO_ENABLED=1
mkdir -p out
go build -trimpath -ldflags "-s -w -buildid=" -o out/AccountChanger-linux-x86_64 .
chmod 0755 out/AccountChanger-linux-x86_64
echo "Linux build -> out/AccountChanger-linux-x86_64"
