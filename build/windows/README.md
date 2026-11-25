# Automata — Windows build

Is folder mein wo sab kuch hai jo Automata ko **Windows `.exe`** mein
badalne ke liye chahiye. Build ke baad ek hi `.exe` double-click karne se:

1. Electron window (UI) khulti hai,
2. C++ backend (`api_server.exe`) khud background mein start hota hai,
3. ChromeDriver bhi khud start hota hai (agar bundle mein mojood ho),
4. UI `http://127.0.0.1:8080/` se load hoti hai — koi manual setup nahi.

---

## Build kya banata hai?

| File | Kya hai |
| --- | --- |
| `dist/Automata-Portable-1.0.0.exe` | **Single `.exe`**, install ke bagair chalta hai (double-click). |
| `dist/Automata-Setup-1.0.0.exe` | Normal installer (Start Menu + Desktop shortcut). |
| `dist/win-unpacked/` | Bina pack kiya app folder (debugging ke liye). |

Dono exe ke andar ye cheezein bundle hoti hain:

```
Automata.exe
resources/
  app/                  <- Electron UI (index.html, js/, css/, main.js)
  backend/api_server.exe <- C++ backend (WinHTTP, koi DLL dependency nahi)
  driver/chromedriver.exe <- browser chalane ke liye
```

`frontend/main.js` packaged app mein `resources/backend/api_server.exe`
khud dhoond kar start karta hai, aur `resources/driver/chromedriver.exe`
bhi. Is liye user ko kuch install nahi karna padta.

---

## 1) Windows par build (native)

Zarurat:

- [Node.js 18+](https://nodejs.org)
- [MSYS2](https://www.msys2.org) + mingw64 g++
  ```bat
  pacman -S --needed mingw-w64-x86_64-gcc
  ```

Phir `cmd` mein:

```bat
cd build\windows
build.bat
```

`build.bat` khud hi:

1. `build-backend.bat` -> `artifacts\backend\api_server.exe`
2. `get-chromedriver.bat` -> `artifacts\driver\chromedriver.exe` (agar pehle se na ho)
3. `npm install` + `electron-builder --win --x64`

Output: `build\windows\dist\*.exe`

---

## 2) Linux / macOS se Windows build (cross-build)

Windows ki zarurat nahi — Zig cross-compiler use hota hai.

```sh
cd build/windows
chmod +x *.sh
./cross-build.sh
```

Ye khud hi Zig download karta hai (`build/windows/.tools/`), backend ko
`x86_64-windows-gnu` ke liye compile karta hai, chromedriver download karta
hai, aur electron-builder se `.exe` banata hai.

Output: `build/windows/dist/*.exe`

---

## Alag alag steps (agar manually chalana ho)

```sh
./build-backend.sh              # sirf api_server.exe
./get-chromedriver.sh           # latest stable driver
./get-chromedriver.sh 131.0.6778.204   # apne Chrome jaisa version
```

```bat
build-backend.bat
get-chromedriver.bat
get-chromedriver.bat 131.0.6778.204
```

Sirf app folder (installer ke bagair) chahiye to:

```sh
cd frontend
npm run pack:win
```

---

## Development mode (Windows)

```bat
cd build\windows
run.bat
```

Ye backend ko `backend\api_server.exe` par copy karke `npm start` chalata hai.

---

## Zaruri baatein

### ChromeDriver version

`chromedriver.exe` ka **major version** user ke Chrome se match karna chahiye
(misaal: Chrome 131 -> chromedriver 131.x). Warna browser wale kaam fail honge
(chat phir bhi chalegi).

Agar driver bundle nahi hai to `frontend/main.js` usay skip kar deta hai.

### Data kahan save hota hai?

Packaged app `%APPDATA%\Automata\` mein likhta hai:

```
%APPDATA%\Automata\
  storage\sessions\*.json   <- chat history
  automata.log              <- app start / backend logs
  backend.log               <- backend ka output
```

### Port

Backend hamesha `127.0.0.1:8080` par sunta hai. Agar pehle se koi Automata
backend chal raha ho to naya nahi start hota — purana hi use hota hai.
Band karne ke liye Task Manager se `api_server.exe` end karo.

### Code signing / SmartScreen

Ye build **unsigned** hai, is liye Windows SmartScreen "Unknown publisher"
dikhata hai -> "More info" -> "Run anyway". Proper sign karne ke liye
`CSC_LINK` / `CSC_KEY_PASSWORD` environment variables set karo aur dobara
`build.bat` / `cross-build.sh` chalao.

### Antivirus / Firewall

Pehli dafa chalane par Windows Firewall localhost ke liye prompt kar sakta
hai — allow kar dena. Kuch antivirus unsigned `.exe` ko quarantine karte hain;
build folder ko exception mein daal do.

### Naam / version / icon badalna

- Naam / version: `frontend/package.json` (`productName`, `version`)
- App id: `frontend/package.json` -> `build.appId`
- Icon: `build/windows/resources/icon.ico` replace karo
  (ya `frontend/icon-512.png` se dobara banao):
  ```sh
  magick frontend/icon-512.png -define icon:auto-resize=256,128,64,48,32,24,16 build/windows/resources/icon.ico
  ```

### `asar` kyun band hai?

`frontend/package.json` mein `"asar": false` hai. C++ backend ko asli files
chahiye (`resources/app/index.html` etc.) — asar ek archive hota hai jise
Electron ke bahar ka program nahi padh sakta. Isi liye asar off hai.

---

## Folder structure

```
build/windows/
  build.bat              # Windows: poora build
  build-backend.bat      # Windows: sirf backend
  get-chromedriver.bat   # Windows: driver download
  run.bat                # Windows: dev mode

  cross-build.sh         # Linux/macOS: poora Windows build
  build-backend.sh       # Linux/macOS: zig se backend
  get-chromedriver.sh    # Linux/macOS: driver download

  resources/icon.ico     # Windows icon (installer + exe)
  artifacts/             # generated: backend + driver (git ignored)
  dist/                  # generated: .exe output (git ignored)
  .tools/                # generated: downloaded zig (git ignored)
```
