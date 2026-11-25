# Automata — Landing Website

The public homepage for Automata. It is a single static page, styled with the
same deep-blue palette as the desktop app.

## What it shows

- The **Automata** wordmark in the centre.
- Above it, a **pixel-art cursor with eyes** that floats, blinks, looks around,
  and has a thinking bubble with animated dots.
- A small music bar at the bottom that plays the files in `music/` in order
  and loops.

## Files

```
website/
├── index.html          # page markup
├── styles.css          # blue theme + pixel cursor + animations
├── app.js              # pixel cursor renderer + music playlist
├── favicon.svg         # browser tab icon
├── vercel.json         # static hosting config (clean URLs, music cache)
└── music/
    ├── playlist.json   # optional ordered list of songs
    └── README.txt      # how to add songs
```

## Add your music

**Easiest:** drop files in `music/` named `1.mp3`, `2.mp3`, `3.mp3`, ...

**Or** edit `music/playlist.json`:

```json
{
  "playlist": ["intro.mp3", "theme.mp3", "outro.mp3"]
}
```

They play in order and loop. Music starts on the first click because browsers
block autoplay.

## Run locally

```sh
cd website
python3 -m http.server 5050
# open http://127.0.0.1:5050
```

## Deploy to Vercel

Live production URL: **https://automata-app.vercel.app**

To redeploy after changes:

```sh
cd website
./deploy.sh
```

Or import the folder in the Vercel dashboard. No build step is needed — it is
plain HTML, CSS, and JS.

## Open automatically in the app

The C++ backend opens this page whenever it starts a new browser session.
Set `AUTOMATA_HOME_URL` to change the address:

```sh
export AUTOMATA_HOME_URL="https://your-deployment.vercel.app"
```

Default value is compiled into `browser_controller.cpp`
(`default_home_url()`).
