"""Compare actual prose and choices with the Ren'Py reference, plus asset bytes."""
import ast
import hashlib
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REFERENCE = ROOT.parent / "renpy" / "game"

def condition(expression):
    match = re.fullmatch(r"(\w+) >= (\d+)", expression)
    if match:
        return {"key": match[1], "op": ">=", "value": int(match[2])}
    return {"key": expression.removeprefix("not "), "op": "==", "value": not expression.startswith("not ")}

def reference_commands(block):
    """Translate only the reference's actual grammar; reject every unknown line."""
    lines = [line.strip() for line in block.splitlines() if line.strip()]
    commands = []
    i = 0
    while i < len(lines):
        line = lines[i]
        i += 1
        if line.startswith("scene bg "):
            cast_line = lines[i]
            i += 1
            cast = re.fullmatch(r"\$ set_cast\((True|False), (True|False)\)", cast_line)
            assert cast, cast_line
            commands.append({"type": "stage", "background": line.removeprefix("scene bg "),
                             "cast": [name for name, value in zip(["mira", "golem"], cast.groups()) if value == "True"]})
        elif line.startswith("play music "):
            path = re.search(r'"([^"]+)"', line)[1]
            commands.append({"type": "music", "track": Path(path).stem})
        elif line.startswith("stop music"):
            commands.append({"type": "stop_music"})
        elif line == "menu:":
            options = []
            while i < len(lines) and lines[i].startswith('"'):
                match = re.fullmatch(r'("(?:[^"\\]|\\.)*")(?: if (.*))?:', lines[i])
                assert match, lines[i]
                target = re.fullmatch(r"jump (\w+)", lines[i + 1])
                assert target, lines[i + 1]
                option = {"text": ast.literal_eval(match[1]), "target": target[1]}
                if match[2]: option["condition"] = condition(match[2])
                options.append(option)
                i += 2
            commands.append({"type": "choice", "options": options})
        elif line.startswith("if "):
            expression = line[3:-1]
            target = re.fullmatch(r"jump (\w+)", lines[i])
            assert target, lines[i]
            i += 1
            commands.append({"type": "if_jump", "condition": condition(expression), "target": target[1]})
        elif line.startswith("jump "):
            commands.append({"type": "jump", "target": line[5:]})
        elif line == "return":
            commands.append({"type": "return"})
        elif line.startswith("call explore_ridge "):
            commands.append({"type": "explore", "place": "ridge"})
        elif line.startswith("$ set_cast("):
            cast = re.fullmatch(r"\$ set_cast\((True|False), (True|False)\)", line)
            assert cast, line
            background = next(c["background"] for c in reversed(commands) if c["type"] == "stage")
            commands.append({"type": "stage", "background": background,
                             "cast": [name for name, value in zip(["mira", "golem"], cast.groups()) if value == "True"]})
        elif line.startswith("$ "):
            statement = line[2:]
            match = re.fullmatch(r"(\w+) (\+=|=) (.+)", statement)
            if match:
                commands.append({"type": "add" if match[2] == "+=" else "set", "key": match[1], "value": ast.literal_eval(match[3])})
            else:
                match = re.fullmatch(r'inventory\.(add|discard)\("([^"]+)"\)', statement)
                assert match, statement
                commands.append({"type": "item_add" if match[1] == "add" else "item_remove", "item": match[2]})
        else:
            match = re.fullmatch(r'(?:(m|g) )?("(?:[^"\\]|\\.)*")', line)
            assert match, line
            commands.append({"type": "say", "speaker": {"m": "mira", "g": "golem", None: ""}[match[1]], "text": ast.literal_eval(match[2])})
    return commands

def main():
    story = json.loads((ROOT / "data/story.json").read_text(encoding="utf-8"))
    source = (REFERENCE / "script.rpy").read_text(encoding="utf-8")
    labels = dict(re.findall(r"(?ms)^label (\w+):\n(.*?)(?=^label |\Z)", source))
    assert set(story["scenes"]) == set(labels) - {"start"}
    for name, commands in story["scenes"].items():
        assert commands == reference_commands(labels[name]), f"Control flow/state/staging mismatch: {name}"
        lines = []
        choices = []
        for line in labels[name].splitlines():
            match = re.match(r'^\s+(?:(m|g) )?("(?:[^"\\]|\\.)*")\s*$', line)
            if match:
                lines.append(({"m": "mira", "g": "golem", None: ""}[match[1]], ast.literal_eval(match[2])))
            match = re.match(r'^\s+("(?:[^"\\]|\\.)*")(?: if .*?)?:$', line)
            if match:
                choices.append(ast.literal_eval(match[1]))
        actual = [(c["speaker"], c["text"]) for c in commands if c["type"] == "say"]
        actual_choices = [o["text"] for c in commands if c["type"] == "choice" for o in c["options"]]
        assert actual == lines, f"Dialogue mismatch: {name}"
        assert actual_choices == choices, f"Choice mismatch: {name}"
        assert commands[0]["type"] == "stage", name
        for c in commands:
            if c["type"] in {"jump", "if_jump"}: assert c["target"] in story["scenes"]
            if c["type"] == "choice":
                assert all(o["target"] in story["scenes"] for o in c["options"])
    for folder in ["images", "audio"]:
        for path in (REFERENCE / folder).rglob("*"):
            if not path.is_file(): continue
            target = ROOT / path.relative_to(REFERENCE)
            assert target.is_file(), f"Missing {target}"
            assert hashlib.sha256(path.read_bytes()).digest() == hashlib.sha256(target.read_bytes()).digest(), target
    print("PASS: all 22 scenes' dialogue, choices, state effects, control flow, staging and music match Ren'Py; seven original media files are byte-identical.")

if __name__ == "__main__":
    main()
