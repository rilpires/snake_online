extends Node

var ws_client : WebSocketPeer
var last_state = null;
var sent_first = false
var packets_queue = []
const MIN_PACKETS_TO_CONSUME = 3

func _ready():
	ws_client = WebSocketPeer.new()
	ws_client.inbound_buffer_size *= 4
	# ws_client.outbound_buffer_size *= 3
	
	var timer = Timer.new()
	timer.wait_time = Globals.POOLING_TIME
	timer.autostart = true
	timer.one_shot = false
	timer.timeout.connect(poll)
	add_child(timer)

func _process(delta):
	var packets_to_consume = max(float(MIN_PACKETS_TO_CONSUME), ceil(0.05*packets_queue.size()) )
	packets_to_consume = int(min(packets_to_consume, packets_queue.size()))
	#if (packets_to_consume > MIN_PACKETS_TO_CONSUME):
	#	print("processing %d out of remaining %d packets" % [packets_to_consume, packets_queue.size()])
	for i in range(0, packets_to_consume):
		packet_received(packets_queue.pop_front())
	

func get_websocket_url():
	if (OS.get_name() == 'Web') :
		var full_url = JavaScriptBridge.eval("window.location.href")
		return (full_url as String).replace("https://", "wss://").replace("http://", "ws://")
	else:
		return 'ws://localhost:8080'


func connect_to_server():
	ws_client.connect_to_url(get_websocket_url())

func poll():
	ws_client.poll()
	var current_state = ws_client.get_ready_state()
	if (current_state != last_state):
		last_state = current_state
		match (current_state) :
			ws_client.STATE_OPEN:
				print("Connection state: STATE_OPEN")
				if not sent_first:
					first_send()
			ws_client.STATE_CLOSED:
				print("Connection state: STATE_CLOSED")
			ws_client.STATE_CLOSING:
				print("Connection state: STATE_CLOSING")
			ws_client.STATE_CONNECTING:
				print("Connection state: STATE_CONNECTING")
	match (current_state) :
		ws_client.STATE_OPEN:
			while (ws_client.get_available_packet_count() > 0):
				var next_packet = ws_client.get_packet()
				var string_packet = next_packet.get_string_from_utf8()
				var json_var = JSON.parse_string(string_packet)
				if json_var != null:
					if json_var is Array:
						packets_queue.append_array(json_var)
					else:
						packets_queue.push_back(json_var)
		ws_client.STATE_CLOSED:
			connect_to_server()

func first_send():
	send_packet({
		"type": "req_lobby_list"
	})
	pass

func packet_received(d:Dictionary):
	Globals.pub(d.type, d)

func send_packet(d:Dictionary, flush=false):
	var err = ws_client.send( JSON.stringify(d).to_utf8_buffer() , WebSocketPeer.WRITE_MODE_TEXT )
	if (err) :
		print(err)
