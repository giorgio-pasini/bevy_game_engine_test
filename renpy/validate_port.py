"""Structural checks for the Ren'Py comparison project.

This does not replace `renpy.exe <project> lint`; it catches missing copied
assets, missing labels, and dangling literal jump/call targets on machines
where the SDK is not installed.
"""

from __future__ import annotations

import re
from pathlib import Path


ROOT = Path(__file__).resolve().parent
GAME = ROOT / "game"

EXPECTED_SCENES = {
    "admitted_lost",
    "after_question",
    "awakening",
    "cairn_marks",
    "ending_carried",
    "ending_carried_name",
    "ending_walked",
    "gave_name",
    "give_gear",
    "introductions",
    "ledge_find",
    "mending",
    "pass_talk",
    "refusal",
    "refusal_lost",
    "refusal_named",
    "stayed_silent",
    "the_name",
    "the_offer",
    "the_question",
    "wander",
    "wander_saw_cairn",
}

EXPECTED_ASSETS = {
    "audio/calm.ogg",
    "audio/mystery.ogg",
    "images/backgrounds/mountains.webp",
    "images/characters/adventurer_character.png",
    "images/characters/golem.png",
    "images/maps/valley.png",
    "images/maps/valley_lookup.png",
}


def main() -> None:
    scripts = sorted(GAME.glob("*.rpy"))
    if not scripts:
        raise SystemExit("no .rpy files found")

    source = "\n".join(path.read_text(encoding="utf-8") for path in scripts)
    labels = set(re.findall(r"(?m)^label\s+([A-Za-z_]\w*)\s*(?:\([^)]*\))?\s*:", source))
    targets = set(re.findall(r"(?m)^\s*(?:jump|call)\s+([A-Za-z_]\w*)\s*$", source))

    missing_scenes = EXPECTED_SCENES - labels
    dangling = targets - labels
    missing_assets = sorted(path for path in EXPECTED_ASSETS if not (GAME / path).is_file())

    errors: list[str] = []
    if missing_scenes:
        errors.append(f"missing scene labels: {sorted(missing_scenes)}")
    if dangling:
        errors.append(f"dangling jump/call targets: {sorted(dangling)}")
    if missing_assets:
        errors.append(f"missing assets: {missing_assets}")
    if "screen valley_map" not in source:
        errors.append("valley map screen is missing")
    if source.count("hotspot (") != 3:
        errors.append("valley map must contain exactly three hotspots")
    if "default persistent.seen_place_options = set()" not in source:
        errors.append("persistent exploration history is missing")
    if "default quick_menu = True" not in source:
        errors.append("quick-menu state is missing")

    if errors:
        raise SystemExit("\n".join(errors))

    print(
        f"Ren'Py port structure OK: {len(EXPECTED_SCENES)} scenes, "
        f"{len(labels)} total labels, {len(EXPECTED_ASSETS)} assets, 3 hotspots."
    )


if __name__ == "__main__":
    main()
