extends SceneTree

var app: Control

func _initialize() -> void:
	run.call_deferred()

func capture(name: String) -> void:
	await create_timer(0.6).timeout
	await process_frame
	await process_frame
	await RenderingServer.frame_post_draw
	var image = root.get_texture().get_image()
	image.save_png("res://test-output/" + name + ".png")
	print("Rendered " + name)

func run() -> void:
	root.content_scale_size = Vector2i(1920, 1080)
	root.content_scale_mode = Window.CONTENT_SCALE_MODE_CANVAS_ITEMS
	app = load("res://main.tscn").instantiate()
	root.add_child(app)
	app.data_dir = "res://test-output/render-data/"
	DirAccess.make_dir_recursive_absolute(app.data_dir)
	app.profile = {"lines": {}, "choices": {}, "places": {}}
	app.prefs.cps = 0
	await capture("main")
	app.new_game()
	await capture("narrator")
	for _i in range(4): app.advance_line()
	await capture("golem")
	for _i in range(20):
		if app.runtime.stop.type == "choice": break
		app.advance_line()
	await capture("choice")
	app.choose(0)
	for _i in range(30):
		if app.runtime.stop.type == "choice": break
		app.advance_line()
	app.choose(0)
	for _i in range(20):
		if app.runtime.stop.type == "explore": break
		app.advance_line()
	await capture("ridge")
	app.choose(3)
	app.map_hover(0)
	await capture("valley")
	app.save_slot(1)
	app.get_node("Notification").hide()
	app.open_menu("Save")
	await capture("save")
	app.open_menu("Preferences")
	await capture("preferences")
	app.open_menu("History")
	await capture("history")
	app.load_slot(1)
	if app.runtime.state.place != "valley":
		push_error("Native save/load failed to restore the map")
		quit(1)
		return
	var thumbnail = Image.load_from_file(app.data_dir + "slot_1.png")
	if thumbnail == null or thumbnail.get_size() != Vector2i(340, 150):
		push_error("Native save thumbnail missing")
		quit(1)
		return
	app.queue_free()
	await process_frame
	await create_timer(0.15).timeout
	quit()
