@echo off
setlocal enableextensions
cd /d "%~dp0"

docker info >nul 2>nul
if errorlevel 1 (
  echo Docker is not running. Start Docker Desktop, wait until it is ready, then re-run.
  pause
  exit /b 1
)

echo Preparing build image ^(first run installs deps, then cached^)...
docker build -t accountchanger-linux-build - < Dockerfile.linux-build
if errorlevel 1 (
  echo Docker image build FAILED
  pause
  exit /b 1
)

if not exist "out" mkdir "out"

echo Building Linux -^> out\AccountChanger-linux-x86_64
docker run --rm -v "%CD%:/src" -v ac-gocache:/root/.cache/go-build -v ac-gomod:/go/pkg/mod -w /src accountchanger-linux-build bash /src/docker-build.sh
if errorlevel 1 (
  echo Build FAILED
  pause
  exit /b 1
)

echo Done: out\AccountChanger-linux-x86_64
pause
