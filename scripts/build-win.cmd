@echo off
rem Windows release build: VS2022 env + writable TEMP (session PATH is trimmed on this box).
set "PATH=C:\Windows\System32;C:\Windows;C:\Users\Administrator\.cargo\bin;%PATH%"
set "TEMP=%~dp0..\tmp\buildtmp"
set "TMP=%~dp0..\tmp\buildtmp"
if not exist "%TEMP%" mkdir "%TEMP%"
call "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat" >nul
set "PATH=C:\Users\Administrator\.cargo\bin;%PATH%"
cd /d "%~dp0.."
cargo build --release -p Phira-Vrenxz-main
if errorlevel 1 exit /b 1
echo BUILD_OK
