style default:
    font "DejaVuSans.ttf"
    size 30
    color "#f0f2f7"

style button:
    background Solid("#1a1e2bea")
    hover_background Solid("#39425cf4")
    selected_background Solid("#20242eea")
    padding (24, 14)

style button_text:
    color "#f0f2f7"
    hover_color "#ffffff"
    selected_color "#8c8f97"
    insensitive_color "#666a73"
    text_align 0.5

style choice_button is button:
    xsize 900
    xalign 0.5

style choice_button_text is button_text:
    size 28

default quick_menu = True

screen say(who, what):
    style_prefix "say"
    window:
        id "window"
        xalign 0.5
        yalign 0.96
        xsize 1660
        yminimum 230
        background Solid("#090b11e8")
        padding (55, 30)

        vbox:
            spacing 12
            if who:
                text who id "who" color "#f8d15a" size 34
            text what id "what" size 32

    use quick_menu

screen choice(items):
    style_prefix "choice"
    vbox:
        xalign 0.5
        yalign 0.48
        spacing 14
        for i in items:
            if i.action:
                textbutton i.caption:
                    action i.action
                    selected i.chosen

screen quick_menu():
    zorder 100
    if quick_menu and not main_menu:
        hbox:
            xalign 0.98
            yalign 0.02
            spacing 8
            textbutton _("Back") action Rollback()
            textbutton _("Save") action ShowMenu("save")
            textbutton _("Load") action ShowMenu("load")
            textbutton _("Prefs") action ShowMenu("preferences")
            textbutton _("History") action ShowMenu("history")
            textbutton _("Auto") action Preference("auto-forward", "toggle")
            textbutton _("Skip") action Skip() alternate Skip(fast=True, confirm=True)
        key "K_F5" action FileSave("1")
        key "K_F9" action FileLoad("1")

screen main_menu():
    tag menu
    add "bg mountains"
    add Solid("#080a11aa")
    frame:
        xalign 0.88
        yalign 0.5
        xsize 620
        background Solid("#0b0e18e8")
        padding (55, 55)
        vbox:
            spacing 18
            text config.name size 58 color "#f8d15a"
            text _("A side-by-side Ren'Py experiment") size 24 color "#b5bac6"
            null height 20
            textbutton _("New Game") action Start()
            if FileLoadable("1"):
                textbutton _("Continue — slot 1") action FileLoad("1")
            textbutton _("Load") action ShowMenu("load")
            textbutton _("Preferences") action ShowMenu("preferences")
            textbutton _("Quit") action Quit(confirm=True)

screen navigation():
    vbox:
        spacing 12
        if not main_menu:
            textbutton _("Return") action Return()
            textbutton _("Save") action ShowMenu("save")
        textbutton _("Load") action ShowMenu("load")
        textbutton _("Preferences") action ShowMenu("preferences")
        textbutton _("History") action ShowMenu("history")
        if not main_menu:
            textbutton _("Main Menu") action MainMenu()
        textbutton _("Quit") action Quit(confirm=True)

screen file_slots(title, load=False):
    tag menu
    add Solid("#0b0e18")
    text title xalign 0.5 ypos 70 size 56 color "#f8d15a"
    frame:
        xpos 80
        ypos 180
        background None
        use navigation
    grid 3 2:
        xalign 0.62
        yalign 0.52
        spacing 24
        for slot in range(1, 7):
            button:
                xsize 380
                ysize 240
                action (FileLoad(str(slot)) if load else FileSave(str(slot)))
                has vbox
                spacing 8
                add FileScreenshot(str(slot)) xalign 0.5 xsize 340 ysize 150
                text FileTime(str(slot), empty=_("Empty slot")) xalign 0.5 size 22
                text FileSaveName(str(slot)) xalign 0.5 size 20

screen save():
    use file_slots(_("Save"))

screen load():
    use file_slots(_("Load"), load=True)

screen preferences():
    tag menu
    add Solid("#0b0e18")
    text _("Preferences") xalign 0.5 ypos 70 size 56 color "#f8d15a"
    frame:
        xpos 80
        ypos 180
        background None
        use navigation
    vbox:
        xalign 0.62
        yalign 0.48
        spacing 24
        textbutton _("Toggle fullscreen") action Preference("display", "toggle")
        text _("Text speed")
        bar value Preference("text speed") xsize 700
        text _("Auto-forward delay")
        bar value Preference("auto-forward time") xsize 700
        text _("Music volume")
        bar value Preference("music volume") xsize 700

screen history():
    tag menu
    add Solid("#0b0e18")
    text _("History") xalign 0.5 ypos 45 size 56 color "#f8d15a"
    frame:
        xpos 60
        ypos 140
        background None
        use navigation
    viewport:
        xpos 390
        ypos 130
        xsize 1420
        ysize 850
        scrollbars "vertical"
        mousewheel True
        draggable True
        vbox:
            spacing 22
            for h in _history_list:
                window:
                    xfill True
                    background Solid("#151a26cc")
                    padding (24, 18)
                    has vbox
                    if h.who:
                        text h.who color "#f8d15a" size 26
                    text h.what substitute False size 28

screen confirm(message, yes_action, no_action):
    modal True
    zorder 200
    add Solid("#000000aa")
    frame:
        xalign 0.5
        yalign 0.5
        xsize 760
        padding (50, 40)
        vbox:
            spacing 28
            text message xalign 0.5 text_align 0.5
            hbox:
                xalign 0.5
                spacing 30
                textbutton _("Yes") action yes_action
                textbutton _("No") action no_action

screen notify(message):
    zorder 300
    frame at notify_appear:
        xalign 0.5
        ypos 40
        text message
    timer 3.25 action Hide("notify")

transform notify_appear:
    alpha 0.0
    linear 0.25 alpha 1.0
    on hide:
        linear 0.5 alpha 0.0

screen skip_indicator():
    zorder 100
    text _("Skipping…") xalign 0.98 yalign 0.94 color "#f8d15a"

screen place_menu(caption, options):
    modal True
    use quick_menu
    frame:
        xalign 0.5
        yalign 0.82
        xsize 960
        background Solid("#090b11e8")
        padding (40, 30)
        vbox:
            spacing 14
            text caption xalign 0.5 color "#f8d15a" size 34
            for caption_text, value, seen_key in options:
                textbutton caption_text:
                    xfill True
                    action [Function(mark_place_seen, seen_key), Return(value)]
                    selected place_seen(seen_key)

screen valley_map():
    modal True
    default hover_text = ""
    use quick_menu
    imagemap:
        ground "images/maps/valley.png"
        hotspot (550, 180, 300, 300):
            action [Function(mark_place_seen, "valley:0"), Return("cairn")]
            hovered SetScreenVariable("hover_text", _("The cairn on the shoulder"))
            unhovered SetScreenVariable("hover_text", "")
        hotspot (1150, 560, 430, 230):
            action [Function(mark_place_seen, "valley:1"), Return("ledge")]
            hovered SetScreenVariable("hover_text", _("The ledge under the ridge"))
            unhovered SetScreenVariable("hover_text", "")
        hotspot (250, 600, 390, 300):
            action [Function(mark_place_seen, "valley:2"), Return("ridge")]
            hovered SetScreenVariable("hover_text", _("Back up to the ridge"))
            unhovered SetScreenVariable("hover_text", "")
    frame:
        xalign 0.5
        yalign 0.92
        background Solid("#090b11e8")
        padding (30, 18)
        text hover_text:
            size 32
            color ("#8c8f97" if valley_hover_seen(hover_text) else "#f0f2f7")
