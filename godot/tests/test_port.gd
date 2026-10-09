extends SceneTree

const Runtime = preload("res://scripts/story_runtime.gd")
var failures: Array[String] = []
var visited: Dictionary = {}
var scenes: Dictionary = {}
var actions: Dictionary = {}
var ends: Dictionary = {}

func _initialize() -> void:
	run.call_deferred()

func check(condition: bool, message: String) -> void:
	if not condition and not failures.has(message): failures.append(message); push_error(message)

func equivalent(a: Variant, b: Variant) -> bool:
	# JSON decodes all numbers as floats; semantic equality permits int/float.
	if a is Dictionary and b is Dictionary:
		if a.size() != b.size(): return false
		for key in a:
			if not b.has(key) or not equivalent(a[key], b[key]): return false
		return true
	if a is Array and b is Array:
		if a.size() != b.size(): return false
		for i in range(a.size()):
			if not equivalent(a[i], b[i]): return false
		return true
	return a == b

func click(position: Vector2) -> void:
	for pressed in [true, false]:
		var event = InputEventMouseButton.new()
		event.position = position
		event.global_position = position
		event.button_index = MOUSE_BUTTON_LEFT
		event.pressed = pressed
		root.push_input(event, true)

func key(code: int) -> void:
	var event = InputEventKey.new()
	event.keycode = code
	event.pressed = true
	root.push_input(event)

func run() -> void:
	root.content_scale_size = Vector2i(1920, 1080)
	root.content_scale_mode = Window.CONTENT_SCALE_MODE_CANVAS_ITEMS
	root.notify_mouse_entered()
	var game = Runtime.new()
	game.advance()
	var queue: Array = [game.snapshot()]
	var cursor = 0
	while cursor < queue.size() and cursor < 15000:
		var data: Dictionary = queue[cursor]
		cursor += 1
		var key: String = JSON.stringify(data)
		if visited.has(key): continue
		visited[key] = true
		game.restore(data)
		scenes[game.state.scene] = true
		check(game.stop.type != "error", "No runtime errors")
		var saved = game.snapshot()
		game.restore(JSON.parse_string(JSON.stringify(saved)))
		check(equivalent(saved, game.snapshot()), "Stable save/load at every stop")
		match game.stop.type:
			"say":
				check(game.stop.speaker == "" or game.state.cast.has(game.stop.speaker), "Speaker is staged: " + game.stop.id)
				game.advance()
				queue.append(game.snapshot())
			"choice", "explore":
				check(not game.stop.options.is_empty(), "No dead ends")
				for i in range(game.stop.options.size()):
					game.restore(data)
					var option: Dictionary = game.stop.options[i]
					if game.stop.type == "explore": actions[option.id] = true
					game.pick(i)
					queue.append(game.snapshot())
			"end": ends[game.state.scene] = true
	check(cursor < 15000, "Finite content graph")
	check(scenes.size() == 22, "All 22 scenes reached: " + str(scenes.keys()))
	check(ends.has("ending_carried") and ends.has("ending_carried_name") and ends.has("ending_walked"), "All ending variants reached")
	check(actions.size() == 13, "All 13 place actions reached: " + str(actions.keys()))
	for path in ["res://images/backgrounds/mountains.webp", "res://images/characters/golem.png", "res://images/characters/adventurer_character.png", "res://images/maps/valley.png", "res://audio/calm.ogg", "res://audio/mystery.ogg", "res://images/DejaVuSans.ttf"]:
		check(ResourceLoader.load(path) != null, "Engine can load asset: " + path)
	var app = load("res://main.tscn").instantiate()
	root.add_child(app)
	await process_frame
	DirAccess.make_dir_recursive_absolute(app.data_dir)
	app.profile = {"lines": {}, "choices": {}, "places": {}}
	app.prefs.cps = 0
	app.new_game()
	await process_frame
	await process_frame
	check(app.runtime.stop.id == "awakening:2", "App opens on first line")
	click(Vector2(500, 900))
	check(app.runtime.stop.id == "awakening:4", "App advances")
	app.go_back()
	check(app.runtime.stop.id == "awakening:2", "Rollback restores previous line")
	check(app.profile.lines.has("awakening:4"), "Rollback retains player knowledge")
	key(KEY_F5)
	app.advance_line()
	key(KEY_F9)
	check(app.runtime.stop.id == "awakening:2", "Real file load returns to saved line")
	check(app.profile.lines.has("awakening:4"), "Loading old slot retains player knowledge")
	for _i in range(30):
		if app.runtime.stop.type == "choice": break
		app.advance_line()
	check(app.runtime.stop.type == "choice", "Intro choices displayed")
	var choice_id: String = app.runtime.stop.options[0].id
	await process_frame
	await process_frame
	click(app.get_node("ChoiceList").get_child(0).get_global_rect().get_center())
	check(app.runtime.state.variables.knows_name, "Real choice sets name flag")
	app.go_back()
	check(app.profile.choices.has(choice_id), "Chosen choice survives rollback")
	var rows: Node = app.get_node("ChoiceList")
	check(rows.get_child(0).get_theme_color("font_color") == Color("#8c8f97"), "Seen choice is visibly muted")
	app.choose(0)
	for _i in range(30):
		if app.runtime.stop.type == "choice": break
		app.advance_line()
	app.choose(0)
	for _i in range(20):
		if app.runtime.stop.type == "explore": break
		app.advance_line()
	check(app.runtime.state.place == "ridge", "Wander calls ridge")
	check(app.runtime.state.track == "mystery", "Music survives exploration")
	check(app.get_node("Music").stream.loop, "Music loops")
	app.choose(3)
	await process_frame
	await process_frame
	click(app.get_node("%Cairn").get_global_rect().get_center())
	check(app.runtime.state.place == "cairn", "Actual map hotspot enters cairn")
	app.choose(0)
	for _i in range(20):
		if app.runtime.stop.type == "explore": break
		app.advance_line()
	check(app.runtime.state.variables.read_cairn, "Cairn action sets knowledge")
	app.choose(1)
	app.choose(1)
	app.choose(0)
	check(app.runtime.state.inventory.has("brass_gear"), "Gear granted on ledge")
	app.save_slot(2)
	app.advance_line()
	app.load_slot(2)
	check(app.runtime.stop.id == "ledge_find:4", "Save restores exact line inside exploration call")
	for _i in range(20):
		if app.runtime.stop.type == "explore": break
		app.advance_line()
	check(app.runtime.stop.options.size() == 1, "Gear cannot be farmed twice")
	app.choose(0)
	app.choose(3)
	for _i in range(20):
		if app.runtime.stop.type == "explore": break
		app.advance_line()
	check(not app.runtime.state.inventory.has("brass_gear"), "Gear consumed")
	check(app.runtime.state.variables.gear_returned, "Gear-return flag set")
	check(app.runtime.state.variables.trust == 6, "Trust reflects name + finding + returning gear")
	app.choose(3)
	await process_frame
	app.map_hover(0)
	check(app.get_node("MapHotspots/MapCaption/Label").get_theme_color("font_color") == Color("#8c8f97"), "Seen hotspot hover visibly muted")
	app.choose(2)
	app.choose(4)
	for _i in range(20):
		if app.runtime.stop.type == "end": break
		app.advance_line()
	check(not app.playing, "Ending returns to main menu")
	app.new_game()
	app.unread_line = false
	app.skipping = true
	app._process(1.0)
	check(app.runtime.stop.id == "awakening:4", "Read-only skip advances read text")
	app.skipping = false
	app.auto_forward = true
	app._process(20.0)
	check(app.runtime.stop.id == "awakening:5", "Auto-forward advances completed text")
	app.auto_forward = false
	app.prefs.cps = 45
	app.new_game()
	app._process(0.1)
	check(app.get_node("%Line").visible_characters > 0 and app.get_node("%Line").visible_characters < 20, "Typewriter progressively reveals text")
	click(Vector2(500, 900))
	check(app.runtime.stop.id == "awakening:2", "Click during typing reveals without skipping line")
	key(KEY_SPACE)
	check(app.runtime.stop.id == "awakening:4", "Space advances revealed dialogue")
	app.open_menu("Save")
	await process_frame
	check(app.get_node("Modal").visible, "Save menu opens")
	app.open_menu("Preferences")
	await process_frame
	app.open_menu("History")
	await process_frame
	app.close_menu()
	var profile_copy: Dictionary = app.profile.duplicate(true)
	app.queue_free()
	await process_frame
	await create_timer(0.15).timeout
	var fresh_app = load("res://main.tscn").instantiate()
	root.add_child(fresh_app)
	await process_frame
	check(equivalent(fresh_app.profile, profile_copy), "Persistent player knowledge survives fresh application")
	fresh_app.queue_free()
	await process_frame
	await create_timer(0.15).timeout
	if failures.is_empty():
		print("PASS: %d states, 22 scenes, 13 place actions, all endings, asset loads, rollback/profile/save UI." % visited.size())
		quit(0)
	else:
		print("FAIL: " + str(failures))
		quit(1)
