@echo off
setlocal
set "ADM_DIST_LAUNCHER=%~dp0dist\AutoDesignMaker-NEWrust\Start-AutoDesignMaker.cmd"
if not exist "%ADM_DIST_LAUNCHER%" (
  echo AutoDesignMaker portable launcher was not found:
  echo   %ADM_DIST_LAUNCHER%
  echo Build or restore the portable dist folder, then try again.
  pause
  exit /b 1
)
call "%ADM_DIST_LAUNCHER%"
endlocal
