MUSIC — how to add your songs
=============================

The landing page plays music in order, then loops.

OPTION A (easiest) — numbered files
-----------------------------------
Put your songs in this folder and name them:

  music/1.mp3
  music/2.mp3
  music/3.mp3
  ...

The page finds them automatically, in numeric order.

OPTION B — playlist.json
------------------------
List the file names in `music/playlist.json`:

  {
    "playlist": ["intro.mp3", "theme.mp3", "outro.mp3"]
  }

Names are relative to the `music/` folder.
The page plays them in the order you write them.

NOTES
-----
- `.mp3` works everywhere. `.ogg` and `.m4a` also work in Chrome.
- Browsers block autoplay until the visitor clicks the page.
  So the first click (or the play button) starts the music.
- Keep file sizes small so the page loads fast.
- Do not delete playlist.json; an empty list is fine.
