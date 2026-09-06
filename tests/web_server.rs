use std::{
    io::{BufRead, BufReader, Write},
    net::TcpStream,
    thread,
    time::Duration,
};
use knot::net::{KnotWebServer, NetworkEvent};
use tungstenite::{connect, Message};

#[test]
fn test_http_web_portal_serves_html() {
    let test_port = 18447;
    let (_server, _rx, _handle) = KnotWebServer::start(test_port);

    thread::sleep(Duration::from_millis(60));

    let mut stream = TcpStream::connect(format!("127.0.0.1:{}", test_port)).expect("Failed to connect HTTP stream");
    let req = "GET / HTTP/1.1\r\nHost: localhost:18447\r\nUser-Agent: Mozilla/5.0\r\n\r\n";
    stream.write_all(req.as_bytes()).unwrap();
    stream.flush().unwrap();

    let mut reader = BufReader::new(stream);
    let mut status_line = String::new();
    reader.read_line(&mut status_line).unwrap();
    assert!(status_line.contains("200 OK"), "Expected HTTP 200 OK, got: {}", status_line);

    let mut body = String::new();
    let mut line = String::new();
    while let Ok(n) = reader.read_line(&mut line) {
        if n == 0 {
            break;
        }
        body.push_str(&line);
        line.clear();
    }

    assert!(body.contains("Project Knot"), "Expected HTML body to contain Project Knot title");
    assert!(body.contains("desktop-canvas"), "Expected HTML body to contain desktop canvas element");
    assert!(body.contains("chat-drawer"), "Expected HTML body to contain floating chat drawer");
}

#[test]
fn test_websocket_handshake_and_events() {
    let test_port = 18448;
    let (_server, rx, handle) = KnotWebServer::start(test_port);

    thread::sleep(Duration::from_millis(60));

    let ws_url = format!("ws://127.0.0.1:{}/ws", test_port);
    let (mut socket, response) = connect(ws_url).expect("WebSocket connection failed");
    assert_eq!(response.status(), 101, "Expected 101 Switching Protocols");

    // 1. Send Join
    let join_msg = serde_json::json!({
        "type": "join",
        "user_name": "Bob (Web Test)",
        "color_hex": "#00E5FF"
    });
    socket.send(Message::Text(join_msg.to_string())).unwrap();

    // 2. Send Pointer Motion
    let motion_msg = serde_json::json!({
        "type": "pointer_motion",
        "x": 720.5,
        "y": 480.0
    });
    socket.send(Message::Text(motion_msg.to_string())).unwrap();

    // 3. Send Pointer Button
    let click_msg = serde_json::json!({
        "type": "pointer_button",
        "button": 272,
        "pressed": true
    });
    socket.send(Message::Text(click_msg.to_string())).unwrap();

    // 4. Send Key Event
    let key_msg = serde_json::json!({
        "type": "key_event",
        "keycode": 38,
        "pressed": true
    });
    socket.send(Message::Text(key_msg.to_string())).unwrap();

    // 5. Send Cursor Chat
    let chat_msg = serde_json::json!({
        "type": "cursor_chat",
        "message": "Web collaboration rocks! 🚀"
    });
    socket.send(Message::Text(chat_msg.to_string())).unwrap();

    // 6. Send Ping & Read Pong
    let ping_msg = serde_json::json!({ "type": "ping" });
    socket.send(Message::Text(ping_msg.to_string())).unwrap();

    thread::sleep(Duration::from_millis(80));

    // Broadcast a test frame
    let fake_jpeg = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46];
    handle.broadcast_frame(&fake_jpeg);

    if let tungstenite::stream::MaybeTlsStream::Plain(s) = socket.get_mut() {
        let _ = s.set_read_timeout(Some(Duration::from_millis(400)));
    }

    // Read messages from WebSocket
    let mut received_pong = false;
    let mut received_frame = false;

    for _ in 0..5 {
        if let Ok(msg) = socket.read() {
            match msg {
                Message::Text(txt) => {
                    if txt.contains("pong") {
                        received_pong = true;
                    }
                }
                Message::Binary(bin) => {
                    if bin == fake_jpeg {
                        received_frame = true;
                    }
                }
                _ => {}
            }
        }
    }

    assert!(received_pong, "Expected to receive pong from server");
    assert!(received_frame, "Expected to receive broadcasted binary frame from server");

    // Check events on server channel
    let mut received_motion = false;
    let mut received_chat = false;

    while let Ok(evt) = rx.try_recv() {
        match evt {
            NetworkEvent::PointerMotion { x, y } => {
                if x == 720.5 && y == 480.0 {
                    received_motion = true;
                }
            }
            NetworkEvent::CursorChat { message } => {
                if message == "Web collaboration rocks! 🚀" {
                    received_chat = true;
                }
            }
            _ => {}
        }
    }

    assert!(received_motion, "Server must receive pointer motion from browser");
    assert!(received_chat, "Server must receive cursor chat from browser");
}
