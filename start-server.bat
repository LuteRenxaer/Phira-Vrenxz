@echo off
rem ============================================================
rem  Phira-MP local multiplayer server launcher
rem  Usage: double-click, or   start-server.bat 50000   (default port 31205)
rem  Ports: game 31205 / web API 31206 / admin+room list 31207 / dglab 31208
rem  NOTE: keep this file ASCII-only. cmd parses .bat bytes with the system
rem        codepage (GBK on zh-CN); UTF-8 Chinese in here can break parsing.
rem ============================================================
chcp 65001 >nul
setlocal
title Phira-MP local server

set "PORT=%~1"
if "%PORT%"=="" set "PORT=31205"

rem server working dir holds log/ record/ user.json bans.json webui/ -> must cd into it
set "ROOT=%~dp0vendor\phira-mp"
set "EXE=%ROOT%\server-src\target\release\phira-mp-server.exe"
if not exist "%EXE%" set "EXE=%ROOT%\target\release\phira-mp-server.exe"

if not exist "%ROOT%" (
    echo [ERROR] working folder not found: %ROOT%
    pause
    exit /b 1
)
if not exist "%EXE%" (
    echo [ERROR] phira-mp-server.exe not found: %EXE%
    echo         build it first:
    echo             cd vendor\phira-mp
    echo             cargo build --release -p phira-mp-server
    pause
    exit /b 1
)

cd /d "%ROOT%"

echo ============================================================
echo   Phira-MP local server
echo ------------------------------------------------------------
echo   exe     : %EXE%
echo   workdir : %ROOT%
echo   port    : %PORT%
echo   client  : set multiplayer server address to 127.0.0.1:%PORT%
echo   stop    : Ctrl+C or close this window
echo ============================================================
echo.

"%EXE%" --port %PORT% --log-level info

echo.
echo [server exited] exit code: %ERRORLEVEL%
echo window kept open so you can read the error above
pause
