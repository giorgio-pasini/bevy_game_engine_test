label explore_ridge:
    scene bg mountains
    $ set_cast(False, False)
    python:
        ridge_options = [
            (_("Walk up to the cairn"), "cairn", "ridge:0"),
            (_("Climb down to the ledge"), "ledge", "ridge:1"),
            (_("Ask the golem about the pass"), "pass_talk", "ridge:2"),
        ]
    if "brass_gear" in inventory:
        $ ridge_options.append((_("Hold out the brass gear"), "give_gear", "ridge:3"))
    python:
        ridge_options.extend([
            (_("Look out over the valley"), "valley", "ridge:4"),
            (_("Go on over the peaks"), "leave", "ridge:5"),
        ])
    call screen place_menu(_("The ridge, above the cloud line"), ridge_options)
    if _return == "cairn":
        jump explore_cairn
    elif _return == "ledge":
        jump explore_ledge
    elif _return == "pass_talk":
        call pass_talk from _call_pass_talk
        jump explore_ridge
    elif _return == "give_gear":
        call give_gear from _call_give_gear
        jump explore_ridge
    elif _return == "valley":
        jump explore_valley
    else:
        return

label explore_cairn:
    scene bg mountains
    $ set_cast(False, False)
    python:
        cairn_options = [
            (_("Read the marks cut into the top stone"), "marks", "cairn:0"),
            (_("Back to the ridge"), "ridge", "cairn:1"),
        ]
    call screen place_menu(_("A cairn of flat stones, shoulder high"), cairn_options)
    if _return == "marks":
        call cairn_marks from _call_cairn_marks
        jump explore_cairn
    jump explore_ridge

label explore_ledge:
    scene bg mountains
    $ set_cast(False, False)
    $ ledge_options = []
    if read_cairn and not found_gear:
        $ ledge_options.append((_("Look for the marked stone the cairn described"), "find", "ledge:0"))
    $ ledge_options.append((_("Climb back up"), "ridge", "ledge:1"))
    call screen place_menu(_("A ledge, barely wide enough to stand on"), ledge_options)
    if _return == "find":
        call ledge_find from _call_ledge_find
        jump explore_ledge
    jump explore_ridge

label explore_valley:
    scene bg valley
    $ set_cast(False, False)
    call screen valley_map
    if _return == "cairn":
        jump explore_cairn
    elif _return == "ledge":
        jump explore_ledge
    jump explore_ridge
