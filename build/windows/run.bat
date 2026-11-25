@echo off
REM ============================================================
REM  Automata — development mode mein chalao (Windows)
REM
REM  Backend + Electron dono isi window se start hote hain.
REM  Pehle build-backend.bat chala chuke ho to behtar hai.
REM ============================================================
setlocal

set "HERE=%~dp0"
set "ROOT=%HERE%..\.."
set "BACKEND_EXE=%HERE%artifacts\backend\api_server.exe"

REM Fresh backend binary ko project ke backend folder mein rakho,
REM taake Electron usay dhoond sake aur storage/ sessions wahan banein.
if exist "%BACKEND_EXE%" (
    if not exist "%ROOT%\backend" mkdir "%ROOT%\backend"
    copy /y "%BACKEND_EXE%" "%ROOT%\backend\api_server.exe" >nul
) else (
    echo WARNING: api_server.exe nahi mila. Pehle build-backend.bat chalao.
)

cd /d "%ROOT%\frontend"
call npm start
endlocal
