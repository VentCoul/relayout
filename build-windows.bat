@echo off
rem Builds Relayout on Windows.
rem   build-windows.bat       - build the installer
rem   build-windows.bat dev   - run in development mode (errors show in this window)
setlocal EnableExtensions
cd /d "%~dp0"
title Relayout - Windows build

echo.
echo === Relayout: Windows build ===
echo.

if exist "%USERPROFILE%\.cargo\bin\cargo.exe" set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"

set "INSTALLED_SOMETHING=0"
set "STILL_MISSING=0"
call :check_tool cargo "Rust" Rustlang.Rustup
call :check_tool node "Node.js" OpenJS.NodeJS.LTS
call :check_msvc
if "%STILL_MISSING%"=="1" goto :missing
if "%INSTALLED_SOMETHING%"=="1" goto :restart

where pnpm >nul 2>nul
if errorlevel 1 (
    echo [setup] Installing pnpm...
    call npm install -g pnpm || goto :fail
)

echo [1/2] Installing JavaScript dependencies...
call pnpm install || goto :fail

if /i "%~1"=="dev" goto :dev

echo [2/2] Building the installer. The first build takes 5-15 minutes...
call pnpm tauri build --bundles nsis || goto :fail
echo.
echo Done. The installer is in target\release\bundle\nsis
explorer "target\release\bundle\nsis"
pause
exit /b 0

:dev
echo [2/2] Starting in development mode. Press Ctrl+C to stop.
call pnpm tauri dev
pause
exit /b 0

:check_tool
where %1 >nul 2>nul
if not errorlevel 1 exit /b 0
echo [setup] %~2 is not installed.
call :can_install || exit /b 0
winget install --id %3 -e --accept-source-agreements --accept-package-agreements
if errorlevel 1 set "STILL_MISSING=1"
set "INSTALLED_SOMETHING=1"
exit /b 0

:check_msvc
set "VSWHERE=%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe"
set "VSPATH="
if not exist "%VSWHERE%" goto :no_msvc
for /f "usebackq delims=" %%i in (`"%VSWHERE%" -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath`) do set "VSPATH=%%i"
if defined VSPATH exit /b 0
:no_msvc
echo [setup] Microsoft C++ Build Tools are not installed (about 6 GB).
call :can_install || exit /b 0
winget install --id Microsoft.VisualStudio.2022.BuildTools -e --accept-source-agreements --accept-package-agreements --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
if errorlevel 1 set "STILL_MISSING=1"
set "INSTALLED_SOMETHING=1"
exit /b 0

:can_install
where winget >nul 2>nul
if errorlevel 1 (
    echo         winget is not available, so it has to be installed by hand.
    set "STILL_MISSING=1"
    exit /b 1
)
choice /c YN /m "        Install it now with winget"
if errorlevel 2 (
    set "STILL_MISSING=1"
    exit /b 1
)
exit /b 0

:restart
echo.
echo New tools were installed. Close this window and run build-windows.bat again
echo so that Windows picks them up.
pause
exit /b 0

:missing
echo.
echo Some required tools are missing. Install them and run this file again:
echo   Rust:             https://rustup.rs
echo   Node.js:          https://nodejs.org
echo   C++ Build Tools:  https://visualstudio.microsoft.com/visual-cpp-build-tools/
echo                     (select "Desktop development with C++")
pause
exit /b 1

:fail
echo.
echo BUILD FAILED. Copy the error text above and send it back.
pause
exit /b 1
