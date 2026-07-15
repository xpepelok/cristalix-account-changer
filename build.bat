@echo off
setlocal

set "OUT=AccountChanger.exe"
set "NEW=%OUT%.new"
set "BACKUP=%OUT%.bak"
set "TRY_GARBLE=0"
set "GOFLAGS=-trimpath"
set "LDFLAGS=-s -w -H=windowsgui -buildid="
set "GARBLE_FLAGS=-literals"
set "CGO_ENABLED=0"
set "GOEXE=go"
set "UPXEXE=upx"
set "GOVERSION=system"
set "GOROOT="
set "GOTOOLCHAIN=local"
set "GOCACHE=%CD%\.gocache-system"
set "GARBLE_CACHE=%CD%\.garble"
set "GOTMPDIR=%CD%\.gotmp"

if /I "%~1"=="garble" set "TRY_GARBLE=1"

if exist "%USERPROFILE%\sdk\go1.26.5\bin\go.exe" set "PATH=%USERPROFILE%\sdk\go1.26.5\bin;%PATH%"
if exist "%USERPROFILE%\sdk\go1.26.5\bin\go.exe" set "GOEXE=%USERPROFILE%\sdk\go1.26.5\bin\go.exe"
if exist "%USERPROFILE%\sdk\go1.26.5\bin\go.exe" set "GOROOT=%USERPROFILE%\sdk\go1.26.5"
if exist "%USERPROFILE%\sdk\go1.26.5\bin\go.exe" set "GOVERSION=go1.26.5"
if exist "%USERPROFILE%\sdk\go1.26.5\bin\go.exe" set "GOCACHE=%CD%\.gocache-go1.26.5"
if exist "%USERPROFILE%\go\bin" set "PATH=%USERPROFILE%\go\bin;%PATH%"
if exist "%CD%\tools\upx\upx.exe" set "UPXEXE=%CD%\tools\upx\upx.exe"

where go >nul 2>nul
if errorlevel 1 (
  if exist "%USERPROFILE%\sdk\go1.25.0\bin\go.exe" set "PATH=%USERPROFILE%\sdk\go1.25.0\bin;%PATH%"
  if exist "%USERPROFILE%\sdk\go1.25.0\bin\go.exe" set "GOEXE=%USERPROFILE%\sdk\go1.25.0\bin\go.exe"
  if exist "%USERPROFILE%\sdk\go1.25.0\bin\go.exe" set "GOROOT=%USERPROFILE%\sdk\go1.25.0"
  if exist "%USERPROFILE%\sdk\go1.25.0\bin\go.exe" set "GOVERSION=go1.25.0"
  if exist "%USERPROFILE%\sdk\go1.25.0\bin\go.exe" set "GOCACHE=%CD%\.gocache-go1.25.0"
  if exist "%ProgramFiles%\Go\bin\go.exe" set "PATH=%ProgramFiles%\Go\bin;%PATH%"
  if exist "%LocalAppData%\Programs\Go\bin\go.exe" set "PATH=%LocalAppData%\Programs\Go\bin;%PATH%"
)

where go >nul 2>nul
if errorlevel 1 (
  echo Go not found in PATH. Install Go or add go.exe to PATH.
  pause
  exit /b 1
)

if not exist "%GOCACHE%" mkdir "%GOCACHE%"
if not exist "%GARBLE_CACHE%" mkdir "%GARBLE_CACHE%"
if not exist "%GOTMPDIR%" mkdir "%GOTMPDIR%"

echo Using %GOVERSION% at %GOROOT%

where garble >nul 2>nul
if errorlevel 1 (
  echo garble not found - installing latest garble...
  "%GOEXE%" install mvdan.cc/garble@latest
  if errorlevel 1 (
    echo Failed to install garble.
    pause
    exit /b 1
  )
  if exist "%USERPROFILE%\go\bin" set "PATH=%USERPROFILE%\go\bin;%PATH%"
)

tasklist /FI "IMAGENAME eq %OUT%" /NH | find /I "%OUT%" >nul
if not errorlevel 1 (
  echo %OUT% is running. Close it before building.
  pause
  exit /b 1
)

if exist "%NEW%" del /f /q "%NEW%" >nul 2>nul
if exist "%NEW%" (
  echo Cannot delete old temporary build %NEW%.
  pause
  exit /b 1
)

if "%TRY_GARBLE%"=="1" (
  echo Building obfuscated release...
  garble %GARBLE_FLAGS% build %GOFLAGS% -ldflags "%LDFLAGS%" -o "%NEW%" .
  if errorlevel 1 (
    echo Garble build failed. Windows Defender may be blocking obfuscated temporary binaries.
    if exist "%NEW%" del /f /q "%NEW%" >nul 2>nul
  )
) else (
  echo Skipping garble. Run build.bat garble to try garble obfuscation.
)

if not exist "%NEW%" (
  echo Building stripped fallback release...
  "%GOEXE%" build %GOFLAGS% -ldflags "%LDFLAGS%" -o "%NEW%" .
  if errorlevel 1 (
    echo Fallback build failed.
    pause
    exit /b 1
  )
  echo Built stripped fallback %NEW%
) else (
  echo Built obfuscated %NEW%
)

if not exist "%UPXEXE%" (
  where upx >nul 2>nul
  if errorlevel 1 (
  echo UPX not found - put upx.exe in tools\upx\ or add it to PATH.
    pause
    exit /b 0
  )
  set "UPXEXE=upx"
)

"%UPXEXE%" --best --lzma "%NEW%"
if errorlevel 1 (
  echo UPX compression failed - keeping uncompressed exe.
  if exist "%NEW%" del /f /q "%NEW%" >nul 2>nul
  pause
  exit /b 1
)

echo Compressed with UPX

if exist "%BACKUP%" del /f /q "%BACKUP%" >nul 2>nul
if exist "%BACKUP%" (
  echo Cannot delete old backup %BACKUP%.
  if exist "%NEW%" del /f /q "%NEW%" >nul 2>nul
  pause
  exit /b 1
)

if exist "%OUT%" (
  ren "%OUT%" "%BACKUP%"
  if errorlevel 1 (
    echo Cannot move %OUT% to %BACKUP%.
    if exist "%NEW%" del /f /q "%NEW%" >nul 2>nul
    pause
    exit /b 1
  )
)

ren "%NEW%" "%OUT%"
if errorlevel 1 (
  echo Cannot move %NEW% to %OUT%.
  if exist "%BACKUP%" if not exist "%OUT%" ren "%BACKUP%" "%OUT%"
  pause
  exit /b 1
)

echo Built %OUT%
pause
