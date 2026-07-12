@echo off
setlocal
set "ADM_WEBVIEW2_FOUND="
if exist "%ProgramFiles(x86)%\Microsoft\EdgeWebView\Application\*\msedgewebview2.exe" set "ADM_WEBVIEW2_FOUND=1"
if exist "%ProgramFiles%\Microsoft\EdgeWebView\Application\*\msedgewebview2.exe" set "ADM_WEBVIEW2_FOUND=1"
if exist "%LOCALAPPDATA%\Microsoft\EdgeWebView\Application\*\msedgewebview2.exe" set "ADM_WEBVIEW2_FOUND=1"
if not defined ADM_WEBVIEW2_FOUND (
  echo Microsoft Edge WebView2 Runtime was not detected.
  echo Install the Evergreen Standalone Runtime, then start AutoDesignMaker again:
  echo https://go.microsoft.com/fwlink/p/?LinkId=2124703
  pause
  exit /b 2
)
set "ADM_NEWRUST_DATA_DIR=%~dp0user_data"
if not defined ADM_NEWRUST_LANGUAGE set "ADM_NEWRUST_LANGUAGE=zh-CN"
start "" /D "%~dp0" "%~dp0AutoDesignMaker.exe"
endlocal
