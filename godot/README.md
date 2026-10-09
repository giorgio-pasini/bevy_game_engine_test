# Golden Golem — Godot / GDScript

Standalone Godot **4.5.1** project recreating the current adventure in
../renpy/game. The original story, character art, background, map and music
are preserved. Gameplay uses GDScript; the small canvas shader reproduces
Ren'Py's inactive-speaker brightness.

## Run and edit

From this directory in PowerShell:

    .\run.ps1
    .\run.ps1 -Editor

The portable editor is already available locally under tools/engine. That
directory is ignored by Git. After a fresh clone, download it with:

    .\tools\setup.ps1

Alternatively pass your own Godot executable using -GodotPath. The launcher
imports assets automatically on the first run. No Rust or .NET installation
is required.

Open project.godot in Godot to edit manually. main.tscn owns the dialogue panel,
character nodes, choice/place panels and the three map hotspot rectangles.
The root node's Inspector exposes character heights, horizontal alignment,
speaker zoom, offsets and dimming. Scripts rebuild dynamic buttons and menu
content; their sizing and colours live in scripts/main.gd. Narrative text,
branches, flags and inventory changes live in data/story.json.

Save the scene in the editor before asking an assistant to change it. Review
the resulting diff before saving an old editor buffer over external changes.

## Controls and features

- Click, Space, Enter: reveal the current line, then advance.
- Back, Page Up, mouse wheel up: rollback.
- F5 / F9: save / load slot 1.
- Save and Load menus: six slots with thumbnails.
- Auto: automatic dialogue advance with adjustable delay.
- Skip / hold Ctrl: skip previously read dialogue, stopping at decisions.
- Right-click Skip: confirm skipping unread dialogue.
- Escape: preferences / return; right-click elsewhere: save menu / return.
- Preferences: text speed (zero = instant), auto delay, music volume, fullscreen.
- History: up to 250 dialogue entries.

All 22 narrative scenes, conditional routes, four exploration locations, trust
and inventory effects, and the three ending labels match the Ren'Py reference.
The valley uses its same three rectangular hotspots. Seen dialogue, narrative
choices and exploration actions live in a separate persistent player profile:
rollback and loading old slots cannot erase them.

Normal player data lives in Godot's user data directory under
%APPDATA%/Godot/app_userdata/Golden Golem - Godot Port. It is separate from
Ren'Py saves, which are not interchangeable.

## Verification

    python .\tools\validate_port.py
    .\run.ps1 -Test
    .\run.ps1 -Render

The reference validator compares every narrative command with the actual
Ren'Py source: dialogue, choices, conditional jumps, variables, inventory,
stage and music. Original media are checked byte-for-byte. The engine suite
walks the complete state graph and exercises the real scene, mouse input,
F5/F9, exploration callbacks, inventory, save/load, rollback, profile history,
skip and auto-forward. Tests use isolated data directories.

The render harness launches a real OpenGL window, captures representative
screens to test-output, and verifies a native save thumbnail and map load.
These captures were inspected against matching Ren'Py renders. Exact pixel
identity is not promised: font rasterisation, resampling and native slider
appearance differ. The Godot preferences screen deliberately keeps every
control visible; the current Ren'Py reference's unstyled bars can push controls
off-screen.

Windows export settings are supplied in export_presets.cfg, including the
external JSON story. Install Godot's matching export templates through the
editor before exporting; a distribution executable has not been built here.

The DejaVuSans font and its licence are in images. Original media retain their
existing provenance, including the visible AI-Generated marks in character art.
