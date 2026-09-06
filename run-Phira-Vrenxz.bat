@echo off
cd /d "%~dp0"
rem Prefer the release build (much faster than debug for huge charts).
if exist "target\release\Phira-Vrenxz-main.exe" (
    echo [RUN] release build
    start "" "target\release\Phira-Vrenxz-main.exe"
) else (
    echo [WARN] release build not found. Build it with: cargo build --release -p Phira-Vrenxz-main
    echo [RUN] fallback to debug build (slow)
    start "" "target\debug\Phira-Vrenxz-main.exe"
)
