# Golden Golem — Ren'Py port

A side-by-side reimplementation of the current Bevy visual-novel adventure.
The Bevy project remains authoritative; this folder exists to measure how much
of the same game Ren'Py supplies directly.

## Run it

This is a complete Ren'Py project directory. With a Ren'Py 8 SDK installed:

```powershell
<renpy-sdk>\renpy.exe D:\giorg\Aziende\GreenAlchemist\GreenNewGroup\GreenNewGames\BevyTest\renpy
```

Or point the Ren'Py Launcher project directory at the repository root and
launch **Golden Golem — Ren'Py Port**. Run the SDK's **Lint** action before
building a distribution.

Verified with Ren'Py 8.5.3: the official lint passes cleanly, and a bounded
native warp-launch at the first dialogue screen remained running without
producing a traceback.

## Controls

- Click, Space or Enter: advance.
- Mouse wheel / Page Up: Ren'Py rollback.
- `F5`: save slot 1.
- `F9`: load slot 1.
- The upper-right quick menu exposes save, load, preferences, history,
  auto-forward and skip.

## Feature parity

| Current Bevy feature | Ren'Py implementation |
| --- | --- |
| 22 narrative scenes and conditional routes | Native labels, `jump`, `call`, `if`, and `menu` |
| Two-character stage and speaker cue | Overlay screen plus character callbacks |
| Persistent music across exploration | Native music channel and saved audio state |
| Story/exploration call stack | Native `call`/`return` stack |
| Four places and conditional actions | Four labels plus one reusable screen |
| Colour-keyed valley map | Three hotspots over the same generated map art |
| Flags, trust counter and inventory | Native rollback-aware store variables |
| Save/load | Native save system; six visible slots plus F5/F9 |
| Seen narrative choices | Native `Choice.chosen`, rendered muted |
| Seen exploration actions | Ren'Py `persistent` set, rendered muted |
| Seen lines / skip eligibility | Native statement-seen tracking |

The map uses rectangular Ren'Py hotspots covering the same visible regions,
not the Bevy engine's exact per-pixel colour lookup. That lookup and its
build-time pixel audit are reusable-engine work, not required game logic in
Ren'Py.

## What Ren'Py supplied before we asked

This port also exposes features that remain later items in the Bevy backlog:

- rollback;
- dialogue history;
- auto-forward;
- skip;
- text-speed, auto-delay, display and music preferences;
- save thumbnails and multiple slots;
- persistent choice history.

`validate_port.py` checks assets and control-flow references without requiring
the SDK:

```powershell
python renpy\validate_port.py
```
