default knows_name = False
default admitted_lost = False
default trust = 0
default read_cairn = False
default found_gear = False
default gear_returned = False
default carries_name = False
default inventory = set()

label start:
    $ set_cast(False, False)
    jump awakening

label awakening:
    scene bg mountains
    $ set_cast(False, False)
    play music "audio/mystery.ogg" fadein 1.0 if_changed

    "Three days up the pass, and the mountain has not once looked back at you."

    $ set_cast(True, False)
    m "Right. Either that ridge moved, or I've been walking in a circle since noon."

    "Then the cloud parts — and something vast turns its head."

    $ set_cast(True, True)
    g "You are small. And you are late."
    m "...Late for what, exactly?"
    g "I have been counting winters since anyone last climbed this far. Six hundred and eleven. You are late by all of them."

    jump introductions

label introductions:
    scene bg mountains
    $ set_cast(True, True)
    g "Speak, then. Travellers used to give a name before they gave demands."

    menu:
        "\"Mira. And I'm not demanding anything.\"":
            jump gave_name
        "Say nothing, and study the gears in his chest.":
            jump stayed_silent
        "\"I'm lost. I'll admit that much.\"":
            jump admitted_lost

label gave_name:
    scene bg mountains
    $ set_cast(True, True)
    $ knows_name = True
    $ trust += 2

    m "Mira. And I'm not demanding anything — I'd just like to know what I'm looking at."
    g "Mira."
    "He says it slowly, the way you'd handle something you were afraid of dropping."
    g "Good. A name is a thing that can be given back later."

    jump the_offer

label stayed_silent:
    scene bg mountains
    $ set_cast(True, True)

    "You keep your mouth shut and your eyes on the machinery: a lattice of gold wheels, most of them still, one of them bent badly out of true."
    g "Careful. Staring is its own kind of demand."
    m "..."
    g "As you like. Silence keeps well up here. Everything does."

    jump the_offer

label admitted_lost:
    scene bg mountains
    $ set_cast(True, True)
    $ admitted_lost = True
    $ trust += 1

    m "I'm lost. I'll admit that much. The map stopped agreeing with the mountain somewhere around dawn."
    g "Maps rarely survive contact with this place. Honesty, though — that is rarer still."
    m "Don't make it into a virtue. I'm just tired."

    jump the_offer

label the_offer:
    scene bg mountains
    $ set_cast(True, True)

    g "There is work in me that has gone unfinished for six hundred years. Gears that no longer turn. A name I can no longer pronounce."
    m "And you want me to what — reach in?"
    g "Mend me, and I will carry you over the peaks by morning. Refuse, and I will let you pass. But you will walk, and the walking is long."

    menu:
        "Look around the ridge first.":
            jump wander
        "Reach for the seized gear in his chest.":
            jump mending
        "\"What happens to you if nobody ever does?\"":
            jump the_question
        "Step back, and keep your hands to yourself.":
            jump refusal

label the_question:
    scene bg mountains
    $ set_cast(True, True)
    $ trust += 2

    m "What happens to you if nobody ever does?"
    "For a long moment the only sound is wind moving through him, the way it moves through a broken window."
    g "I keep counting. That is all. The counting does not stop when the turning does."
    m "That's a horrible answer."
    g "You asked a horrible question. I thought it deserved the truth."

    jump after_question

label after_question:
    scene bg mountains
    $ set_cast(True, True)

    menu:
        "Reach for the seized gear after all.":
            jump mending
        "Step back. This is not your repair to make.":
            jump refusal
        "\"Tell me the name you can't pronounce. I'll carry it for you.\"" if trust >= 3:
            jump the_name

label the_name:
    scene bg mountains
    $ set_cast(True, True)
    $ carries_name = True
    play music "audio/calm.ogg" fadeout 1.0 fadein 1.0 if_changed

    g "You would carry it? It is heavy, and it is not a kind thing to know."
    m "I've carried worse up this mountain. Try me."
    "He tells her. It takes some time, and it is not a word in any language she has met."
    m "...That's it? That's the whole thing?"
    g "That is the whole thing. Six hundred years, and it took eleven seconds to hand over."

    jump mending

label wander:
    scene bg mountains
    $ set_cast(True, True)

    m "Give me a minute. I don't decide things this size standing still."

    call explore_ridge from _call_explore_ridge

    scene bg mountains
    $ set_cast(True, True)
    g "Well. You have had your minute, and rather more of them than that."

    if read_cairn:
        jump wander_saw_cairn

    m "It's a big mountain. Fine. Let's do this."
    jump mending

label wander_saw_cairn:
    scene bg mountains
    $ set_cast(True, True)
    $ trust += 2

    m "Someone counted every winter you did. Six hundred and eleven marks, cut by hand, and then they stopped."
    g "I know."
    m "You said nobody had climbed this far."
    g "Nobody had. They were already here."
    "He does not elaborate, and she decides — for now — not to make him."

    jump mending

label pass_talk:
    scene bg mountains
    $ set_cast(True, True)

    m "You said you'd carry me over. Do you actually know the way, or are we both guessing?"
    g "I know every way. I have had six hundred years and nothing to walk them with."
    m "That's not a yes."
    g "It is the closest thing to one you will get above the cloud line."

    return

label cairn_marks:
    scene bg mountains
    $ set_cast(True, True)
    $ read_cairn = True

    "Someone has cut a line of shallow marks into the top stone. Not letters. Tallies."
    m "Six hundred and eleven."
    g "I did not cut those."
    m "...Then who was counting you?"
    "The last mark points down, over the edge, toward a ledge she had not noticed."

    return

label ledge_find:
    scene bg mountains
    $ set_cast(True, True)
    $ found_gear = True
    $ trust += 1
    $ inventory.add("brass_gear")

    "The marked stone comes away easily. Behind it, wrapped in oiled cloth, is a brass gear no bigger than her palm — and unmistakably the shape of the one seized in his chest."
    m "Someone left this here. For you. And then stopped coming."
    g "...Yes."
    g "Bring it up. Please."
    "It is the first time he has asked for anything."

    return

label give_gear:
    scene bg mountains
    $ set_cast(True, True)

    "She holds it up. Small, filthy, and unmistakably his."
    g "...Where."
    m "Under a marked stone on the ledge. Wrapped, so it wouldn't corrode. Someone meant you to have it back."
    "He is quiet for long enough that she wonders whether she has broken something worse than a gear."
    g "Put it in. Please. I find I cannot ask twice."

    $ inventory.discard("brass_gear")
    $ gear_returned = True
    $ trust += 3

    "It seats with a sound like a held breath let go."

    return

label mending:
    scene bg mountains
    $ set_cast(True, True)
    play music "audio/calm.ogg" fadeout 1.0 fadein 1.0 if_changed

    "The bent wheel resists, complains, and finally gives. Something deep in the gold starts keeping time with her pulse."
    g "Ah. Warm hands. I had forgotten the shape of that."

    if not knows_name:
        jump ending_carried

    g "Thank you, Mira."
    m "You remembered."
    g "I have had a great deal of practice at counting. A name is easier."

    jump ending_carried

label refusal:
    scene bg mountains
    $ set_cast(True, True)
    stop music fadeout 1.0

    m "No. I don't know what I'd be starting, and neither do you."
    g "Wise. Cowardly, but wise. Not every hand that opens me means to close me again."

    if knows_name:
        jump refusal_named
    if admitted_lost:
        jump refusal_lost

    g "Go, then. Stranger you came, stranger you leave. It is the usual arrangement."
    jump ending_walked

label refusal_named:
    scene bg mountains
    $ set_cast(True, True)

    g "Go, then, Mira. I will keep the name. It costs me nothing to hold, and I have the room."
    m "That's not remotely comforting."
    g "No. But it is true, and you seemed to prefer those."

    jump ending_walked

label refusal_lost:
    scene bg mountains
    $ set_cast(True, True)

    g "Go, then. You said you were lost. Follow the water down — it is less honest than you are, but it knows the way."
    m "...Thank you."

    jump ending_walked

label ending_carried:
    scene bg mountains
    $ set_cast(True, False)

    "He carries her over the peaks by morning, exactly as promised."

    if carries_name:
        jump ending_carried_name

    "Neither of them says anything about the winters. Below the pass the map starts agreeing with the mountain again, which Mira decides not to mention."

    return

label ending_carried_name:
    scene bg mountains
    $ set_cast(True, False)

    "She says the name once, near the summit, mostly to check that she still has it. The wind takes it and gives nothing back."
    "Below the pass she stops counting the winters he told her about, and starts counting the ones ahead. It is a better arithmetic, and it is his now as much as hers."

    return

label ending_walked:
    scene bg mountains
    $ set_cast(True, False)

    "The cloud closes over him as she passes. She walks."
    "The walking is long, and she is alone with the sound of her own gears — and, for rather longer than she expects, with the arithmetic of six hundred and eleven winters."

    return

