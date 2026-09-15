init offset = -2

init python:
    gui.init(1920, 1080)

define gui.text_font = "DejaVuSans.ttf"
define gui.name_text_font = "DejaVuSans.ttf"
define gui.interface_text_font = "DejaVuSans.ttf"

define gui.text_size = 32
define gui.name_text_size = 34
define gui.interface_text_size = 30
define gui.label_text_size = 42
define gui.title_text_size = 72

define gui.text_color = "#f0f2f7"
define gui.name_text_color = "#f8d15a"
define gui.interface_text_color = "#f0f2f7"
define gui.accent_color = "#f8d15a"
define gui.idle_color = "#b5bac6"
define gui.hover_color = "#ffffff"
define gui.selected_color = "#f8d15a"
define gui.insensitive_color = "#666a73"

define config.default_text_cps = 45
define config.default_afm_time = 15

image bg mountains:
    "images/backgrounds/mountains.webp"
    xysize (1920, 1080)
    fit "cover"

image bg valley:
    "images/maps/valley.png"
    xysize (1920, 1080)
    fit "contain"

image mira portrait:
    "images/characters/adventurer_character.png"
    ysize 596
    fit "contain"

image golem portrait:
    "images/characters/golem.png"
    ysize 745
    fit "contain"

default mira_visible = False
default golem_visible = False
default speaking = None

default persistent.seen_place_options = set()

init python:
    def speaker_callback(character_id, event, interact=True, **kwargs):
        if event == "begin":
            store.speaking = character_id
            if interact:
                renpy.restart_interaction()
        elif event == "end":
            store.speaking = None

    def set_cast(mira=False, golem=False):
        store.mira_visible = mira
        store.golem_visible = golem
        store.speaking = None
        renpy.restart_interaction()

    def place_seen(key):
        return key in persistent.seen_place_options

    def mark_place_seen(key):
        if key not in persistent.seen_place_options:
            persistent.seen_place_options.add(key)
            renpy.save_persistent()

    def valley_hover_seen(hover_text):
        if not hover_text:
            return False
        lowered = hover_text.lower()
        if "cairn" in lowered:
            return place_seen("valley:0")
        if "ledge" in lowered:
            return place_seen("valley:1")
        return place_seen("valley:2")

    def mira_callback(event, interact=True, **kwargs):
        speaker_callback("mira", event, interact, **kwargs)

    def golem_callback(event, interact=True, **kwargs):
        speaker_callback("golem", event, interact, **kwargs)

    def narrator_callback(event, interact=True, **kwargs):
        speaker_callback(None, event, interact, **kwargs)

define m = Character(
    "Mira",
    color="#f3c969",
    callback=mira_callback,
)
define g = Character(
    "The Golden Golem",
    color="#d9a937",
    callback=golem_callback,
)
define narrator = Character(
    None,
    callback=narrator_callback,
)

transform cast_left(active=True):
    xalign 0.22
    yalign 1.0
    zoom (1.04 if active else 1.0)
    yoffset (-156 if active else -130)
    matrixcolor BrightnessMatrix(0.0 if active else -0.45)

transform cast_right(active=True):
    xalign 0.78
    yalign 1.0
    zoom (1.04 if active else 1.0)
    yoffset (-156 if active else -130)
    matrixcolor BrightnessMatrix(0.0 if active else -0.45)

screen stage_cast():
    zorder -10
    if not main_menu:
        if mira_visible:
            add "mira portrait" at cast_left(speaking is None or speaking == "mira")
        if golem_visible:
            add "golem portrait" at cast_right(speaking is None or speaking == "golem")

init python:
    config.overlay_screens.append("stage_cast")
