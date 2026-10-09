class_name StoryRuntime
extends RefCounted

const DEFAULTS = {"knows_name": false, "admitted_lost": false, "trust": 0,
	"read_cairn": false, "found_gear": false, "gear_returned": false, "carries_name": false}
var story: Dictionary
var state: Dictionary
var stop: Dictionary = {}

func _init() -> void:
	story = JSON.parse_string(FileAccess.get_file_as_string("res://data/story.json"))
	new_game()

func new_game() -> void:
	state = {"scene": story.start, "index": 0, "variables": DEFAULTS.duplicate(),
		"inventory": [], "stack": [], "mode": "story", "place": "ridge",
		"background": "mountains", "cast": [], "track": ""}
	stop = {}

func holds(c: Dictionary) -> bool:
	var value: Variant = state.variables.get(c.get("key", ""))
	match c.get("op", "=="):
		"==": return value == c.value
		"!=": return value != c.value
		">=": return float(value) >= float(c.value)
	return false

func jump(target: String) -> void:
	state.scene = target
	state.index = 0
	state.mode = "story"

func return_frame() -> void:
	if state.stack.is_empty():
		state.mode = "end"
	else:
		var frame: Dictionary = state.stack.pop_back()
		state.mode = frame.mode
		if frame.mode == "story":
			state.scene = frame.scene
			state.index = frame.index
		else:
			state.place = frame.place

func advance() -> Dictionary:
	for _guard in range(1000):
		if state.mode == "explore":
			stop = place_stop()
			return stop
		if state.mode == "end":
			stop = {"type": "end"}
			return stop
		var commands: Array = story.scenes[state.scene]
		if int(state.index) >= commands.size():
			return_frame()
			continue
		var index: int = int(state.index)
		var cmd: Dictionary = commands[index]
		state.index = index + 1
		match cmd.type:
			"stage":
				state.background = cmd.background
				state.cast = cmd.cast.duplicate()
			"music": state.track = cmd.track
			"stop_music": state.track = ""
			"set": state.variables[cmd.key] = cmd.value
			"add": state.variables[cmd.key] += cmd.value
			"item_add":
				if not state.inventory.has(cmd.item): state.inventory.append(cmd.item)
			"item_remove": state.inventory.erase(cmd.item)
			"jump": jump(cmd.target)
			"if_jump":
				if holds(cmd.condition): jump(cmd.target)
			"say":
				stop = cmd.duplicate(true)
				stop.id = "%s:%d" % [state.scene, index]
				return stop
			"choice":
				stop = {"type": "choice", "options": []}
				for i in range(cmd.options.size()):
					var option: Dictionary = cmd.options[i]
					if not option.has("condition") or holds(option.condition):
						var copy: Dictionary = option.duplicate(true)
						copy.id = "%s:%d:%d" % [state.scene, index, i]
						stop.options.append(copy)
				return stop
			"explore":
				state.stack.append({"mode": "story", "scene": state.scene, "index": state.index})
				state.mode = "explore"
				state.place = cmd.place
			"return": return_frame()
			_:
				push_error("Unknown story command: " + str(cmd))
				stop = {"type": "error"}
				return stop
	push_error("Story exceeded command limit")
	return {"type": "error"}

func pick(index: int) -> Dictionary:
	if index < 0 or index >= stop.get("options", []).size(): return stop
	var option: Dictionary = stop.options[index]
	if stop.type == "choice":
		jump(option.target)
	else:
		match option.kind:
			"place": state.place = option.target
			"call":
				state.stack.append({"mode": "explore", "place": state.place})
				jump(option.target)
			"leave": return_frame()
	return advance()

func action(id: String, text: String, kind: String, target: String = "") -> Dictionary:
	return {"id": id, "text": text, "kind": kind, "target": target}

func place_stop() -> Dictionary:
	state.background = "valley" if state.place == "valley" else "mountains"
	state.cast = []
	var result = {"type": "explore", "place": state.place, "caption": "", "options": []}
	match state.place:
		"ridge":
			result.caption = "The ridge, above the cloud line"
			result.options = [action("ridge:0", "Walk up to the cairn", "place", "cairn"),
				action("ridge:1", "Climb down to the ledge", "place", "ledge"),
				action("ridge:2", "Ask the golem about the pass", "call", "pass_talk")]
			if state.inventory.has("brass_gear"):
				result.options.append(action("ridge:3", "Hold out the brass gear", "call", "give_gear"))
			result.options.append(action("ridge:4", "Look out over the valley", "place", "valley"))
			result.options.append(action("ridge:5", "Go on over the peaks", "leave"))
		"cairn":
			result.caption = "A cairn of flat stones, shoulder high"
			result.options = [action("cairn:0", "Read the marks cut into the top stone", "call", "cairn_marks"),
				action("cairn:1", "Back to the ridge", "place", "ridge")]
		"ledge":
			result.caption = "A ledge, barely wide enough to stand on"
			if state.variables.read_cairn and not state.variables.found_gear:
				result.options.append(action("ledge:0", "Look for the marked stone the cairn described", "call", "ledge_find"))
			result.options.append(action("ledge:1", "Climb back up", "place", "ridge"))
		"valley":
			result.options = [action("valley:0", "The cairn on the shoulder", "place", "cairn"),
				action("valley:1", "The ledge under the ridge", "place", "ledge"),
				action("valley:2", "Back up to the ridge", "place", "ridge")]
	return result

func snapshot() -> Dictionary:
	return {"state": state.duplicate(true), "stop": stop.duplicate(true)}

func restore(data: Dictionary) -> void:
	state = data.state.duplicate(true)
	stop = data.stop.duplicate(true)
