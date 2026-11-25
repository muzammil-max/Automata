@echo off
REM ============================================================
REM  Automata — Windows ke liye chromedriver.exe download karo
REM
REM  Istemaal:
REM      get-chromedriver.bat                 REM latest stable
REM      get-chromedriver.bat 131.0.6778.204  REM apne Chrome jaisa
REM
REM  Output: build\windows\artifacts\driver\chromedriver.exe
REM ============================================================
setlocal

set "HERE=%~dp0"
set "OUT_DIR=%HERE%artifacts\driver"
set "VERSION=%~1"

if not exist "%OUT_DIR%" mkdir "%OUT_DIR%"

if "%VERSION%"=="" (
    echo ==^> Latest stable chromedriver ka version dhoonda ja raha hai...
    powershell -NoProfile -ExecutionPolicy Bypass -Command ^
      "$ProgressPreference='SilentlyContinue';" ^
      "$meta = Invoke-RestMethod 'https://googlechromelabs.github.io/chrome-for-testing/last-known-good-versions-with-downloads.json';" ^
      "$v = $meta.channels.Stable.version;" ^
      "$u = ($meta.channels.Stable.downloads.chromedriver | Where-Object { $_.platform -eq 'win64' } | Select-Object -First 1).url;" ^
      "Write-Host ('==^> chromedriver ' + $v + ' download ho raha hai...');" ^
      "Invoke-WebRequest -Uri $u -OutFile '%TEMP%\chromedriver.zip';" ^
      "Expand-Archive -Force '%TEMP%\chromedriver.zip' '%TEMP%\chromedriver-unzip';" ^
      "Copy-Item -Force (Get-ChildItem -Recurse -Filter chromedriver.exe '%TEMP%\chromedriver-unzip' | Select-Object -First 1).FullName '%OUT_DIR%\chromedriver.exe'"
) else (
    powershell -NoProfile -ExecutionPolicy Bypass -Command ^
      "$ProgressPreference='SilentlyContinue';" ^
      "$u = 'https://storage.googleapis.com/chrome-for-testing-public/%VERSION%/win64/chromedriver-win64.zip';" ^
      "Write-Host ('==^> chromedriver %VERSION% download ho raha hai...');" ^
      "Invoke-WebRequest -Uri $u -OutFile '%TEMP%\chromedriver.zip';" ^
      "Expand-Archive -Force '%TEMP%\chromedriver.zip' '%TEMP%\chromedriver-unzip';" ^
      "Copy-Item -Force (Get-ChildItem -Recurse -Filter chromedriver.exe '%TEMP%\chromedriver-unzip' | Select-Object -First 1).FullName '%OUT_DIR%\chromedriver.exe'"
)

if errorlevel 1 (
    echo ERROR: chromedriver download fail ho gaya.
    echo        Manually bhi rakh sakte hain:
    echo        %OUT_DIR%\chromedriver.exe
    exit /b 1
)

echo.
echo OK: %OUT_DIR%\chromedriver.exe
echo     Yaad rahe: iska major version aapke Chrome se match karna chahiye.
endlocal
