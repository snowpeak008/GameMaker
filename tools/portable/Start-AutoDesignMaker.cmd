@echo off
setlocal
set "ADM_WEBVIEW2_FOUND="
for /d %%D in ("%ProgramFiles(x86)%\Microsoft\EdgeWebView\Application\*") do if exist "%%~fD\msedgewebview2.exe" set "ADM_WEBVIEW2_FOUND=1"
for /d %%D in ("%ProgramFiles%\Microsoft\EdgeWebView\Application\*") do if exist "%%~fD\msedgewebview2.exe" set "ADM_WEBVIEW2_FOUND=1"
for /d %%D in ("%LOCALAPPDATA%\Microsoft\EdgeWebView\Application\*") do if exist "%%~fD\msedgewebview2.exe" set "ADM_WEBVIEW2_FOUND=1"
if /i "%~1"=="--check-webview2" (
  if defined ADM_WEBVIEW2_FOUND exit /b 0
  exit /b 2
)
if not defined ADM_WEBVIEW2_FOUND (
  echo Microsoft Edge WebView2 Runtime was not detected.
  echo Install the Evergreen Standalone Runtime, then start AutoDesignMaker again:
  echo https://go.microsoft.com/fwlink/p/?LinkId=2124703
  pause
  exit /b 2
)
set "ADM_NEWRUST_DATA_DIR=%~dp0user_data"
if not defined ADM_NEWRUST_STARTUP_PROJECT set "ADM_NEWRUST_STARTUP_PROJECT=blank"
if not defined ADM_NEWRUST_LANGUAGE set "ADM_NEWRUST_LANGUAGE=zh-CN"
start "" /D "%~dp0" "%~dp0AutoDesignMaker.exe"
if errorlevel 1 (
  echo AutoDesignMaker could not be started from:
  echo   %~dp0AutoDesignMaker.exe
  pause
  exit /b 3
)
endlocal
