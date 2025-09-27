extends Node

const POOLING_TIME = 0.015
var IS_TOUCH_SCREEN = false
var my_client_id = "";

func _init():
	pass

var pubsub_board := {}
var last_value_board := {}
func pub(channel:String , arg = null):
	last_value_board[channel] = arg
	if not pubsub_board.has(channel):
		return
	pubsub_board[channel] = (pubsub_board[channel] as Array).filter(
		func(id): return is_instance_id_valid(id)
	)
	for id in pubsub_board[channel]:
		var obj = instance_from_id(id)
		obj.call(channel, arg)

func sub(who:Object, channel:String, arg = null):
	if (who.is_queued_for_deletion()):
		return false
	if not pubsub_board.has(channel):
		pubsub_board[channel] = []
	if not (pubsub_board[channel] as Array).has(who.get_instance_id()):
		(pubsub_board[channel] as Array).push_back(who.get_instance_id())
	return true

var entries_templates = {};
var entries_parents_templates = {}
func replicate_entry(groupname:String, data:Array, node_builder:Callable):
	var template = entries_templates.get(groupname)
	var all_nodes = get_tree().get_nodes_in_group(groupname)
	var parent = entries_parents_templates.get(groupname)
	if template == null:
		if all_nodes.size() == 0:
			printerr("no template found")
			return
		else:
			template = all_nodes[0] as Node
			entries_templates[groupname] = template
			parent = template.get_parent() as Node;
			entries_parents_templates[groupname] = parent
			parent.remove_child(template)
			all_nodes = get_tree().get_nodes_in_group(groupname)
	if template != null and parent != null and parent.is_inside_tree():
		for node in all_nodes:
			node.queue_free()
		for d in data:
			var new_inst = template.duplicate()
			parent.add_child(node_builder.call(new_inst, d))
	

func focus_game_input():
	get_viewport().gui_release_focus()
