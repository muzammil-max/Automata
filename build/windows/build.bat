@echo off
REM ============================================================
REM  Automata — Windows par poora app banao
REM
REM  Zarurat:
REM    - Node.js 18+      https://nodejs.org
REM    - MSYS2 + mingw64 g++  (backend ke liye)
REM        pacman -S --needed mingw-w64-x86_64-gcc
REM
REM  Output: build\windows\dist\
REM    Automata-Setup-1.0.0.exe      -> installer
REM    Automata-Portable-1.0.0.exe   -> bina install chalao
REM ============================================================
setlocal

set "HERE=%~dp0"
set "ROOT=%HERE%..\.."

echo ============================================================
echo  1/3  Backend (api_server.exe)
echo ============================================================
call "%HERE%build-backend.bat"
if errorlevel 1 goto :fail

echo.
echo ============================================================
echo  2/3  ChromeDriver
echo ============================================================
if exist "%HERE%artifacts\driver\chromedriver.exe" (
    echo ==^> Pehle se mojood hai, skip.
) else (
    call "%HERE%get-chromedriver.bat"
    if errorlevel 1 echo WARNING: chromedriver download fail hua, app phir bhi banegi.
)

echo.
echo ============================================================
echo  3/3  Electron Windows package
echo ============================================================
cd /d "%ROOT%\frontend"
call npm install --no-audit --no-fund
if errorlevel 1 goto :fail

call npx electron-builder --win --x64 --publish never
if errorlevel 1 goto :fail

echo.
echo ============================================================
echo  TAYYAR
echo ============================================================
dir /b "%HERE%dist\*.exe"
echo.
echo  Automata-Setup-*.exe     -^> install karo, Start Menu se chalao
echo  Automata-Portable-*.exe  -^> seedha double-click (backend khud start hota hai)
endlocal
exit /b 0

:fail
echo.
echo ERROR: build fail ho gaya.
endlocal
exit /b 1
