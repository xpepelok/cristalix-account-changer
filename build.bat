@echo off
setlocal enableextensions
cd /d "%~dp0"

set "GOTOOLCHAIN=local"
set "GOTMPDIR=%CD%\.gotmp"
if not exist "%GOTMPDIR%" mkdir "%GOTMPDIR%"

set "GOROOT="
set "GO="
for %%R in ("%USERPROFILE%\sdk\go1.26.5" "%USERPROFILE%\sdk\go1.25.0") do (
  if not defined GO if exist "%%~R\bin\go.exe" (
    set "GO=%%~R\bin\go.exe"
    set "GOROOT=%%~R"
  )
)
if not defined GO set "GO=go"

"%GO%" version >nul 2>nul
if errorlevel 1 (
  echo Go not found. Install Go 1.25+ or add go.exe to PATH.
  pause
  exit /b 1
)

if not exist "out" mkdir "out"

echo Building Windows -^> out\AccountChanger.exe
"%GO%" build -trimpath -ldflags "-s -w -H=windowsgui" -o "out\AccountChanger.exe" .
if errorlevel 1 (
  echo Build FAILED
  pause
  exit /b 1
)

echo Done: out\AccountChanger.exe
pause
