extends Control

const Runtime = preload("res://scripts/story_runtime.gd")
const GOLD = Color("#f8d15a")
const TEXT = Color("#f0f2f7")
const MUTED = Color("#8c8f97")
const NAMES = {"": "", "mira": "Mira", "golem": "The Golden Golem"}
const BACKGROUNDS = {"mountains": "res://images/backgrounds/mountains.webp", "valley": "res://images/maps/valley.png"}
@export_group("Character presentation")
@export var mira_height: float = 596.0
@export var golem_height: float = 745.0
@export_range(0.0, 1.0) var mira_alignment: float = 0.22
@export_range(0.0, 1.0) var golem_alignment: float = 0.78
@export var active_zoom: float = 1.04
@export var active_bottom_offset: float = 156.0
@export var inactive_bottom_offset: float = 130.0
@export var inactive_brightness: float = -0.45
var runtime = Runtime.new()
var rollback: Array = []
var history: Array = []
var profile: Dictionary = {"lines": {}, "choices": {}, "places": {}}
var prefs: Dictionary = {"cps": 45.0, "auto_delay": 15.0, "volume": 1.0, "fullscreen": false}
var playing = false
var auto_forward = false
var skipping = false
var fast_skip = false
var unread_line = true
var reveal = 0.0
var elapsed = 0.0
var track = ""
var music_tween: Tween
var data_dir = "user://"
var last_game_image: Image
var modal_kind = ""

func _ready() -> void:
	mouse_filter = Control.MOUSE_FILTER_IGNORE
	for arg in OS.get_cmdline_user_args():
		if arg.begins_with("--data-dir="): data_dir = arg.trim_prefix("--data-dir=").trim_suffix("/") + "/"
	DirAccess.make_dir_recursive_absolute(data_dir)
	profile = read_json(data_dir + "profile.json", profile)
	prefs = read_json(data_dir + "preferences.json", prefs)
	setup_theme()
	setup_signals()
	%Mira.texture = load("res://images/characters/adventurer_character.png")
	%Golem.texture = load("res://images/characters/golem.png")
	for sprite in [%Mira, %Golem]:
		var material = ShaderMaterial.new()
		material.shader = load("res://scripts/speaker_brightness.gdshader")
		sprite.material = material
	%Background.texture = load(BACKGROUNDS.mountains)
	apply_preferences()
	show_main()
	layout_main.call_deferred()

func _exit_tree() -> void:
	for child in get_children():
		if child is AudioStreamPlayer: child.stop()

func box(color: Color, left: float = 24, top: float = 14) -> StyleBoxFlat:
	var result = StyleBoxFlat.new()
	result.bg_color = color
	result.content_margin_left = left
	result.content_margin_right = left
	result.content_margin_top = top
	result.content_margin_bottom = top
	return result

func setup_theme() -> void:
	var t = Theme.new()
	t.default_font = load("res://images/DejaVuSans.ttf")
	t.default_font_size = 30
	t.set_color("font_color", "Label", TEXT)
	t.set_color("font_color", "Button", TEXT)
	t.set_color("font_hover_color", "Button", Color.WHITE)
	t.set_color("font_focus_color", "Button", Color.WHITE)
	t.set_stylebox("normal", "Button", box(Color("#1a1e2bea")))
	t.set_stylebox("hover", "Button", box(Color("#39425cf4")))
	t.set_stylebox("pressed", "Button", box(Color("#20242eea")))
	t.set_stylebox("focus", "Button", box(Color("#39425c88")))
	t.set_stylebox("panel", "PanelContainer", box(Color("#090b11e8"), 0, 0))
	theme = t
	%Speaker.add_theme_font_size_override("font_size", 34)
	%Speaker.add_theme_color_override("font_color", GOLD)
	%Line.add_theme_font_size_override("normal_font_size", 32)
	%Line.add_theme_color_override("default_color", TEXT)
	%Caption.add_theme_font_size_override("font_size", 34)
	%Caption.add_theme_color_override("font_color", GOLD)
	%Title.add_theme_font_size_override("font_size", 56)
	%Title.add_theme_color_override("font_color", GOLD)
	$MainMenu/Margin/Rows/Title.add_theme_font_size_override("font_size", 58)
	$MainMenu/Margin/Rows/Title.add_theme_color_override("font_color", GOLD)
	$MainMenu/Margin/Rows/Title.text = "Golden Golem — Godot Port"
	$MainMenu/Margin/Rows/Title.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	$MainMenu/Margin/Rows/Title.custom_minimum_size.x = 510
	%MainMenu.add_theme_stylebox_override("panel", box(Color("#0b0e18e8"), 0, 0))
	for node in $MainMenu/Margin/Rows.get_children():
		if node is Button: node.size_flags_horizontal = Control.SIZE_SHRINK_BEGIN
	$MainMenu/Margin/Rows/Subtitle.add_theme_font_size_override("font_size", 24)
	$MainMenu/Margin/Rows/Subtitle.add_theme_color_override("font_color", Color("#b5bac6"))
	%SkipIndicator.add_theme_color_override("font_color", GOLD)
	for node in [%Cairn, %Ledge, %Ridge]:
		for key in ["normal", "hover", "pressed", "focus"]:
			node.add_theme_stylebox_override(key, StyleBoxEmpty.new())
		node.mouse_default_cursor_shape = Control.CURSOR_POINTING_HAND
	%Line.mouse_filter = Control.MOUSE_FILTER_IGNORE
	%Dialogue.mouse_filter = Control.MOUSE_FILTER_IGNORE
	%MapCaption.mouse_filter = Control.MOUSE_FILTER_IGNORE
	$Dialogue/Margin.add_theme_constant_override("margin_top", 30)
	$Dialogue/Margin.add_theme_constant_override("margin_bottom", 30)
	$Dialogue/Margin/Rows.add_theme_constant_override("separation", 12)
	%Modal.position = Vector2.ZERO
	%Modal.size = Vector2(1920, 1080)
	%Modal.add_theme_stylebox_override("panel", StyleBoxEmpty.new())
	%ModalShade.color = Color("#0b0e18")
	$Modal/Margin/Rows/Header.hide()
	for side in ["left", "right", "top", "bottom"]:
		$Modal/Margin.add_theme_constant_override("margin_" + side, 0)
	$Modal/Margin/Rows.add_theme_constant_override("separation", 0)

func setup_signals() -> void:
	$MainMenu/Margin/Rows/NewGame.pressed.connect(new_game)
	%Continue.pressed.connect(func(): load_slot(1))
	$MainMenu/Margin/Rows/Load.pressed.connect(func(): open_menu("Load"))
	$MainMenu/Margin/Rows/Preferences.pressed.connect(func(): open_menu("Preferences"))
	$MainMenu/Margin/Rows/Quit.pressed.connect(func(): confirm_action("Quit the game?", func(): get_tree().quit()))
	$QuickMenu/Back.pressed.connect(go_back)
	$QuickMenu/Save.pressed.connect(func(): open_menu("Save"))
	$QuickMenu/Load.pressed.connect(func(): open_menu("Load"))
	$QuickMenu/Prefs.pressed.connect(func(): open_menu("Preferences"))
	$QuickMenu/History.pressed.connect(func(): open_menu("History"))
	$QuickMenu/Auto.pressed.connect(func(): auto_forward = not auto_forward; elapsed = 0; update_quick())
	$QuickMenu/Skip.pressed.connect(func(): fast_skip = false; skipping = not skipping; update_quick())
	$QuickMenu/Skip.gui_input.connect(func(event: InputEvent):
		if event is InputEventMouseButton and event.pressed and event.button_index == MOUSE_BUTTON_RIGHT:
			$QuickMenu/Skip.accept_event()
			confirm_action("Skip unread dialogue?", func(): fast_skip = true; skipping = true; update_quick()))
	%Close.pressed.connect(close_menu)
	%NotifyTimer.timeout.connect(func(): %Notification.hide())
	var nodes = [%Cairn, %Ledge, %Ridge]
	for i in range(nodes.size()):
		var index: int = i
		nodes[i].pressed.connect(func(): choose(index))
		nodes[i].mouse_entered.connect(func(): map_hover(index))
		nodes[i].mouse_exited.connect(func(): %MapCaption/Label.text = "")

func _process(delta: float) -> void:
	if not playing or %Modal.visible or runtime.stop.get("type") != "say": return
	var total: int = %Line.get_total_character_count()
	if %Line.visible_characters >= 0 and %Line.visible_characters < total:
		if skipping and (fast_skip or not unread_line):
			%Line.visible_characters = total
			return
		reveal += delta * float(prefs.cps)
		%Line.visible_characters = total if float(prefs.cps) <= 0 else mini(int(reveal), total)
		return
	elapsed += delta
	if skipping and (fast_skip or not unread_line) and elapsed > 0.08:
		advance_line()
	elif auto_forward and elapsed > maxf(0.5, float(prefs.auto_delay) * float(total + 25) / 250.0):
		advance_line()

func _unhandled_input(event: InputEvent) -> void:
	if event is InputEventKey and event.pressed and not event.echo:
		match event.keycode:
			KEY_F5:
				if playing: save_slot(1)
			KEY_F9: load_slot(1)
			KEY_ESCAPE:
				if %Modal.visible: close_menu()
				elif playing: open_menu("Preferences")
			KEY_PAGEUP: go_back()
			KEY_SPACE, KEY_ENTER: advance_line()
			KEY_CTRL:
				fast_skip = false
				skipping = true
				update_quick()
	if event is InputEventKey and not event.pressed and event.keycode == KEY_CTRL:
		skipping = false
		update_quick()
	if event is InputEventMouseButton and event.pressed:
		if event.button_index == MOUSE_BUTTON_WHEEL_UP: go_back()
		elif event.button_index == MOUSE_BUTTON_LEFT: advance_line()
		elif event.button_index == MOUSE_BUTTON_RIGHT:
			if %Modal.visible: close_menu()
			elif playing: open_menu("Save")

func new_game() -> void:
	close_menu()
	runtime.new_game()
	history.clear()
	rollback.clear()
	playing = true
	auto_forward = false
	skipping = false
	fast_skip = false
	runtime.advance()
	accept_stop()

func show_main() -> void:
	playing = false
	close_menu()
	%MainMenu.show()
	%MainShade.show()
	%Continue.visible = FileAccess.file_exists(slot_path(1))
	for node in [%Mira, %Golem, %Dialogue, %PlacePanel, %MapHotspots, %ChoiceList, %QuickMenu, %SkipIndicator]: node.hide()
	%Background.texture = load(BACKGROUNDS.mountains)
	sync_music("")

func accept_stop() -> void:
	if runtime.stop.type == "end":
		show_main()
		return
	if runtime.stop.type == "say":
		unread_line = not profile.lines.has(runtime.stop.id)
		profile.lines[runtime.stop.id] = true
		history.append({"speaker": runtime.stop.speaker, "text": runtime.stop.text})
		if history.size() > 250: history.pop_front()
		save_profile()
	rollback.append(session_snapshot())
	if rollback.size() > 500: rollback.pop_front()
	render()

func advance_line() -> void:
	if not playing or %Modal.visible or runtime.stop.type != "say": return
	if %Line.visible_characters >= 0 and %Line.visible_characters < %Line.get_total_character_count():
		%Line.visible_characters = %Line.get_total_character_count()
		reveal = %Line.get_total_character_count()
		elapsed = 0
		return
	runtime.advance()
	accept_stop()

func choose(index: int) -> void:
	if %Modal.visible or not playing: return
	var options: Array = runtime.stop.get("options", [])
	if index < 0 or index >= options.size(): return
	var kind = "choices" if runtime.stop.type == "choice" else "places"
	profile[kind][options[index].id] = true
	save_profile()
	skipping = false
	fast_skip = false
	runtime.pick(index)
	accept_stop()

func session_snapshot() -> Dictionary:
	return {"runtime": runtime.snapshot(), "history": history.duplicate(true), "music_position": %Music.get_playback_position()}

func restore_session(data: Dictionary) -> void:
	runtime.restore(data.runtime)
	history = data.get("history", []).duplicate(true)
	playing = true
	auto_forward = false
	skipping = false
	fast_skip = false
	unread_line = false
	close_menu()
	render()
	if %Music.playing: %Music.seek(float(data.get("music_position", 0.0)))

func go_back() -> void:
	if not playing or %Modal.visible or rollback.size() < 2: return
	rollback.pop_back()
	restore_session(rollback.back())
	%Line.visible_characters = -1

func button(text: String, callback: Callable, seen: bool = false) -> Button:
	var b = Button.new()
	b.text = text
	b.pressed.connect(callback)
	if seen: b.add_theme_color_override("font_color", MUTED)
	return b

func clear_children(node: Node) -> void:
	for child in node.get_children():
		node.remove_child(child)
		child.queue_free()

func render() -> void:
	%MainMenu.hide()
	%MainShade.hide()
	%QuickMenu.show()
	%Background.texture = load(BACKGROUNDS[runtime.state.background])
	sync_music(runtime.state.track)
	var speaking: String = runtime.stop.get("speaker", "")
	for pair in [["mira", %Mira, mira_height, mira_alignment], ["golem", %Golem, golem_height, golem_alignment]]:
		var node: TextureRect = pair[1]
		node.visible = runtime.state.cast.has(pair[0])
		var active: bool = speaking.is_empty() or speaking == pair[0]
		var h: float = float(pair[2]) * (active_zoom if active else 1.0)
		var w: float = h * float(node.texture.get_width()) / float(node.texture.get_height())
		node.size = Vector2(w, h)
		node.position = Vector2((1920.0 - w) * float(pair[3]), 1080.0 - h - (active_bottom_offset if active else inactive_bottom_offset))
		node.material.set_shader_parameter("brightness", 0.0 if active else inactive_brightness)
	%Dialogue.visible = runtime.stop.type == "say"
	%ChoiceList.visible = runtime.stop.type == "choice"
	%PlacePanel.visible = runtime.stop.type == "explore" and runtime.state.place != "valley"
	%MapHotspots.visible = runtime.stop.type == "explore" and runtime.state.place == "valley"
	if runtime.stop.type == "say":
		%Speaker.text = NAMES[speaking]
		%Speaker.visible = not speaking.is_empty()
		%Line.text = runtime.stop.text
		reveal = 0
		elapsed = 0
		%Line.visible_characters = -1 if float(prefs.cps) <= 0 else 0
		if unread_line and not fast_skip: skipping = false
	elif runtime.stop.type == "choice":
		clear_children(%ChoiceList)
		for i in range(runtime.stop.options.size()):
			var index: int = i
			var option: Dictionary = runtime.stop.options[i]
			var b = button(option.text, func(): choose(index), profile.choices.has(option.id))
			b.add_theme_font_size_override("font_size", 28)
			%ChoiceList.add_child(b)
		layout_choices.call_deferred()
	elif runtime.stop.type == "explore":
		if runtime.state.place == "valley":
			%MapCaption/Label.text = ""
		else:
			%Caption.text = runtime.stop.caption
			clear_children(%Options)
			for i in range(runtime.stop.options.size()):
				var index: int = i
				var option: Dictionary = runtime.stop.options[i]
				var b = button(option.text, func(): choose(index), profile.places.has(option.id))
				b.alignment = HORIZONTAL_ALIGNMENT_LEFT
				%Options.add_child(b)
			layout_place.call_deferred()
	update_quick()

func layout_choices() -> void:
	%ChoiceList.size.y = %ChoiceList.get_combined_minimum_size().y
	%ChoiceList.position.y = (1080 - %ChoiceList.size.y) * 0.48

func layout_place() -> void:
	%PlacePanel.size.y = %PlacePanel.get_combined_minimum_size().y
	%PlacePanel.position.y = (1080 - %PlacePanel.size.y) * 0.82

func map_hover(index: int) -> void:
	if runtime.stop.type != "explore" or runtime.state.place != "valley": return
	var option: Dictionary = runtime.stop.options[index]
	%MapCaption/Label.text = option.text
	%MapCaption/Label.add_theme_color_override("font_color", MUTED if profile.places.has(option.id) else TEXT)

func update_quick() -> void:
	$QuickMenu/Auto.add_theme_color_override("font_color", GOLD if auto_forward else TEXT)
	$QuickMenu/Skip.add_theme_color_override("font_color", GOLD if skipping else TEXT)
	%SkipIndicator.visible = skipping and playing

func sync_music(next: String) -> void:
	if track == next: return
	if music_tween: music_tween.kill()
	track = next
	if next.is_empty():
		music_tween = create_tween()
		music_tween.tween_property(%Music, "volume_db", -60.0, 1.0)
		music_tween.tween_callback(func(): %Music.stop())
	else:
		if %Music.playing:
			var outgoing = AudioStreamPlayer.new()
			outgoing.stream = %Music.stream
			outgoing.volume_db = %Music.volume_db
			add_child(outgoing)
			outgoing.play(%Music.get_playback_position())
			var fade = create_tween()
			fade.tween_property(outgoing, "volume_db", -60.0, 1.0)
			fade.tween_callback(outgoing.queue_free)
		%Music.stream = load("res://audio/%s.ogg" % next)
		%Music.stream.loop = true
		%Music.volume_db = -60
		%Music.play()
		music_tween = create_tween()
		music_tween.tween_property(%Music, "volume_db", linear_to_db(maxf(float(prefs.volume), 0.001)), 1.0)

func apply_preferences() -> void:
	%Music.volume_db = linear_to_db(maxf(float(prefs.volume), 0.001))
	DisplayServer.window_set_mode(DisplayServer.WINDOW_MODE_FULLSCREEN if prefs.fullscreen else DisplayServer.WINDOW_MODE_WINDOWED)

func read_json(path: String, fallback: Dictionary = {}) -> Dictionary:
	if not FileAccess.file_exists(path): return fallback.duplicate(true)
	var value: Variant = JSON.parse_string(FileAccess.get_file_as_string(path))
	return value if value is Dictionary else fallback.duplicate(true)

func write_json(path: String, data: Dictionary) -> bool:
	var file = FileAccess.open(path + ".tmp", FileAccess.WRITE)
	if file == null: return false
	file.store_string(JSON.stringify(data))
	file.flush()
	var error = file.get_error()
	file.close()
	if error != OK: return false
	return DirAccess.rename_absolute(path + ".tmp", path) == OK

func save_profile() -> void:
	if not write_json(data_dir + "profile.json", profile): notify("Could not save profile")

func slot_path(slot: int) -> String:
	return data_dir + "slot_%d.json" % slot

func save_slot(slot: int) -> void:
	if not playing: return
	var data = {"version": 1, "time": Time.get_datetime_string_from_system().replace("T", " "),
		"session": session_snapshot(), "rollback": rollback.duplicate(true)}
	if not write_json(slot_path(slot), data):
		notify("Could not save")
		return
	if DisplayServer.get_name() != "headless":
		var image: Image = last_game_image if %Modal.visible else get_viewport().get_texture().get_image()
		if image != null:
			image = image.duplicate()
			image.resize(340, 150)
			image.save_png(data_dir + "slot_%d.png" % slot)
	notify("Saved — slot %d" % slot)
	if modal_kind == "Save": open_menu("Save")

func load_slot(slot: int) -> void:
	var data = read_json(slot_path(slot))
	if data.get("version") != 1 or not data.has("session"):
		notify("Empty or incompatible slot")
		return
	rollback = data.get("rollback", []).duplicate(true)
	restore_session(data.session)
	notify("Loaded — slot %d" % slot)

func notify(message: String) -> void:
	%Notification/Label.text = message
	%Notification.show()
	%NotifyTimer.start()

func close_menu() -> void:
	%Modal.hide()
	%ModalShade.hide()
	modal_kind = ""
	elapsed = 0

func open_menu(kind: String) -> void:
	if not %Modal.visible and DisplayServer.get_name() != "headless":
		last_game_image = get_viewport().get_texture().get_image()
	modal_kind = kind
	%Modal.show()
	%ModalShade.show()
	clear_children(%ModalBody)
	var title_label = label(kind, 56)
	title_label.add_theme_color_override("font_color", GOLD)
	title_label.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	title_label.position = Vector2(0, 70)
	title_label.size = Vector2(1920, 80)
	%ModalBody.add_child(title_label)
	var row = VBoxContainer.new()
	row.position = Vector2(80, 180)
	row.add_theme_constant_override("separation", 12)
	%ModalBody.add_child(row)
	row.add_child(button("Return", close_menu))
	if playing: row.add_child(button("Save", func(): open_menu("Save")))
	for menu in ["Load", "Preferences", "History"]:
		var menu_name: String = menu
		row.add_child(button(menu_name, func(): open_menu(menu_name)))
	if playing: row.add_child(button("Main Menu", func(): confirm_action("Return to the main menu?", show_main)))
	row.add_child(button("Quit", func(): confirm_action("Quit the game?", func(): get_tree().quit())))
	for node in row.get_children():
		node.size_flags_horizontal = Control.SIZE_SHRINK_BEGIN
	match kind:
		"Save", "Load": build_slots(kind == "Load")
		"Preferences": build_preferences()
		"History": build_history()

func layout_main() -> void:
	%MainMenu.size.y = %MainMenu.get_combined_minimum_size().y
	%MainMenu.position = Vector2((1920 - 620) * 0.88, (1080 - %MainMenu.size.y) * 0.5)

func build_slots(loading: bool) -> void:
	var grid = GridContainer.new()
	grid.columns = 3
	grid.add_theme_constant_override("h_separation", 24)
	grid.add_theme_constant_override("v_separation", 24)
	grid.position = Vector2(454, 300)
	%ModalBody.add_child(grid)
	for slot in range(1, 7):
		var slot_id: int = slot
		var data = read_json(slot_path(slot))
		var b = button("", func():
			if loading: load_slot(slot_id)
			elif FileAccess.file_exists(slot_path(slot_id)): confirm_action("Overwrite slot %d?" % slot_id, func(): save_slot(slot_id))
			else: save_slot(slot_id))
		b.custom_minimum_size = Vector2(380, 240)
		b.disabled = loading and data.is_empty()
		grid.add_child(b)
		var rows = VBoxContainer.new()
		rows.position = Vector2(24, 14)
		rows.size = Vector2(340, 212)
		rows.mouse_filter = Control.MOUSE_FILTER_IGNORE
		b.add_child(rows)
		var picture = TextureRect.new()
		picture.custom_minimum_size = Vector2(340, 150)
		picture.expand_mode = TextureRect.EXPAND_IGNORE_SIZE
		picture.stretch_mode = TextureRect.STRETCH_KEEP_ASPECT_CENTERED
		picture.mouse_filter = Control.MOUSE_FILTER_IGNORE
		var path = data_dir + "slot_%d.png" % slot
		if FileAccess.file_exists(path): picture.texture = ImageTexture.create_from_image(Image.load_from_file(path))
		rows.add_child(picture)
		var timestamp = label(str(data.get("time", "Empty slot")), 22)
		timestamp.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
		rows.add_child(timestamp)
		if not data.is_empty():
			rows.add_child(label(str(data.session.runtime.state.scene).replace("_", " "), 20))

func label(text: String, font_size: int = 30) -> Label:
	var result = Label.new()
	result.text = text
	result.add_theme_font_size_override("font_size", font_size)
	result.mouse_filter = Control.MOUSE_FILTER_IGNORE
	return result

func build_preferences() -> void:
	var content = VBoxContainer.new()
	content.position = Vector2(756, 300)
	content.size.x = 700
	content.add_theme_constant_override("separation", 24)
	%ModalBody.add_child(content)
	content.add_child(button("Toggle fullscreen", func():
		prefs.fullscreen = not prefs.fullscreen
		apply_preferences()
		write_json(data_dir + "preferences.json", prefs)))
	for spec in [["Text speed", "cps", 0.0, 120.0], ["Auto-forward delay", "auto_delay", 1.0, 30.0], ["Music volume", "volume", 0.0, 1.0]]:
		content.add_child(label(spec[0]))
		var slider = HSlider.new()
		slider.custom_minimum_size = Vector2(700, 45)
		slider.min_value = spec[2]
		slider.max_value = spec[3]
		slider.step = 0.01 if spec[1] == "volume" else 1.0
		slider.value = prefs[spec[1]]
		var key: String = spec[1]
		slider.value_changed.connect(func(value: float):
			prefs[key] = value
			apply_preferences()
			write_json(data_dir + "preferences.json", prefs))
		content.add_child(slider)

func build_history() -> void:
	var scroll = ScrollContainer.new()
	scroll.position = Vector2(390, 130)
	scroll.size = Vector2(1420, 850)
	%ModalBody.add_child(scroll)
	var rows = VBoxContainer.new()
	rows.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	rows.add_theme_constant_override("separation", 22)
	scroll.add_child(rows)
	for entry in history:
		var panel = PanelContainer.new()
		panel.add_theme_stylebox_override("panel", box(Color("#151a26cc"), 24, 18))
		var texts = VBoxContainer.new()
		panel.add_child(texts)
		if not entry.speaker.is_empty():
			var speaker_label = label(NAMES[entry.speaker], 26)
			speaker_label.add_theme_color_override("font_color", GOLD)
			texts.add_child(speaker_label)
		var line = label(entry.text, 28)
		line.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
		line.size_flags_horizontal = Control.SIZE_EXPAND_FILL
		texts.add_child(line)
		rows.add_child(panel)

func confirm_action(message: String, yes: Callable) -> void:
	var previous = modal_kind
	%Modal.show()
	%ModalShade.show()
	clear_children(%ModalBody)
	modal_kind = "Confirm"
	var content = VBoxContainer.new()
	content.position = Vector2(580, 400)
	content.size = Vector2(760, 280)
	content.add_theme_constant_override("separation", 28)
	%ModalBody.add_child(content)
	content.add_child(label(message))
	content.add_child(button("Yes", func():
		if not previous.is_empty(): open_menu(previous)
		else: close_menu()
		yes.call()))
	content.add_child(button("No", func():
		if not previous.is_empty(): open_menu(previous)
		else: close_menu()))
