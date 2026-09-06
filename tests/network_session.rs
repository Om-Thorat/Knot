use std::{
    io::{BufRead, BufReader, Write},
    net::TcpStream,
    thread,
    time::Duration,
};
use knot::net::{
    proto::{ClientMessage, ServerMessage},
    KnotNetworkServer, NetworkEvent,
};

#[test]
fn test_live_network_session_lifecycle() {
    let test_port = 17447;
    let (server, rx) = KnotNetworkServer::start(test_port);

    // Wait 50ms for server socket to bind
    thread::sleep(Duration::from_millis(50));

    // Connect test client
    let mut stream = TcpStream::connect(format!("127.0.0.1:{}", test_port)).expect("Client connect failed");
    stream.set_nodelay(true).unwrap();

    // 1. Send Join message
    let join = ClientMessage::Join {
        user_name: "Bob (Remote)".to_string(),
        color_hex: "#00E5FF".to_string(),
    };
    stream.write_all(join.to_json_line().unwrap().as_bytes()).unwrap();
    stream.flush().unwrap();

    // 2. Read Welcome response
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    let welcome = ServerMessage::from_json_line(&line).expect("Expected welcome packet");
    match welcome {
        ServerMessage::Welcome { assigned_seat_name, user_color, .. } => {
            assert_eq!(assigned_seat_name, "seat-bob");
            assert_eq!(user_color, "#00E5FF");
        }
        _ => panic!("Expected Welcome message, got: {:?}", welcome),
    }

    // 3. Send Pointer Motion
    let motion = ClientMessage::PointerMotion { x: 600.0, y: 350.0 };
    stream.write_all(motion.to_json_line().unwrap().as_bytes()).unwrap();
    stream.flush().unwrap();

    // 4. Send Cursor Chat
    let chat = ClientMessage::CursorChat { message: "Hello Knot!".to_string() };
    stream.write_all(chat.to_json_line().unwrap().as_bytes()).unwrap();
    stream.flush().unwrap();

    // Verify events arrived at server channel
    thread::sleep(Duration::from_millis(100));
    let mut received_motion = false;
    let mut received_chat = false;

    while let Ok(evt) = rx.try_recv() {
        match evt {
            NetworkEvent::PointerMotion { x, y } => {
                if x == 600.0 && y == 350.0 {
                    received_motion = true;
                }
            }
            NetworkEvent::CursorChat { message } => {
                if message == "Hello Knot!" {
                    received_chat = true;
                }
            }
            _ => {}
        }
    }

    assert!(received_motion, "Server must receive remote pointer motion");
    assert!(received_chat, "Server must receive remote cursor chat");
}
