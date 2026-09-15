define config.name = _("Golden Golem — Ren'Py Port")
define build.name = "GoldenGolemRenPy"
define config.version = "0.1.0"

define gui.show_name = True
define config.save_directory = "GoldenGolemRenPy-20260819"
define config.has_sound = True
define config.has_music = True
define config.has_voice = False
define config.history_length = 250
define config.check_conflicting_properties = True

define config.enter_transition = dissolve
define config.exit_transition = dissolve
define config.intra_transition = dissolve
define config.after_load_transition = dissolve
define config.end_game_transition = dissolve

init python:
    build.classify("**~", None)
    build.classify("**.bak", None)
    build.classify("**/.**", None)
    build.classify("game/**.rpy", "archive")
    build.classify("game/**.rpyc", "archive")
    build.classify("game/**.webp", "archive")
    build.classify("game/**.png", "archive")
    build.classify("game/**.ogg", "archive")
