@echo off
REM ============================================================
REM  Automata — backend ko Windows par native build karo
REM
REM  Zarurat: MSYS2 (https://www.msys2.org) ke saath mingw64 g++.
REM  MSYS2 install karne ke baad "MSYS2 MinGW 64-bit" shell kholo
REM  aur ek dafa ye chalao:
REM      pacman -S --needed mingw-w64-x86_64-gcc
REM
REM  Output: build\windows\artifacts\backend\api_server.exe
REM ============================================================
setlocal

set "ROOT=%~dp0..\.."
set "BACKEND=%ROOT%\backend"
set "OUT_DIR=%ROOT%\build\windows\artifacts\backend"

if not exist "%OUT_DIR%" mkdir "%OUT_DIR%"

where g++ >nul 2>&1
if errorlevel 1 (
    echo ERROR: g++ nahi mila.
    echo        MSYS2 install karo aur "MSYS2 MinGW 64-bit" shell se chalao.
    echo        pacman -S --needed mingw-w64-x86_64-gcc
    exit /b 1
)

cd /d "%BACKEND%"

echo ==^> api_server.exe build ho raha hai...
g++ -std=c++17 -O2 -Wall -Wextra ^
    main.cpp ^
    api\api_server.cpp ^
    api\nvidia_client.cpp ^
    api\http_client.cpp ^
    agent\agent.cpp ^
    browser\browser_controller.cpp ^
    parser\parser.cpp ^
    storage\session_storage.cpp ^
    instructions\instruction.cpp ^
    -lwinhttp -lws2_32 -luser32 ^
    -o "%OUT_DIR%\api_server.exe"

if errorlevel 1 (
    echo.
    echo ERROR: build fail ho gaya.
    exit /b 1
)

echo.
echo OK: %OUT_DIR%\api_server.exe
dir "%OUT_DIR%\api_server.exe"
endlocal
