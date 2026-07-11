@echo off
setlocal
set "ADM_NEWRUST_SOURCE_ROOT=%~dp0"
set "ADM_NEWRUST_DATA_DIR=%~dp0user_data"
if not defined ADM_NEWRUST_LANGUAGE set "ADM_NEWRUST_LANGUAGE=zh-CN"
start "" /D "%~dp0" "%~dp0AutoDesignMaker.exe"
endlocal
