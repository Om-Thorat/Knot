use knot::net::proto::{ClientMessage, ServerMessage};

#[test]
fn test_client_message_serialization() {
    let join = ClientMessage::Join {
        user_name: "Bob (Guest)".to_string(),
        color_hex: "#00E5FF".to_string(),
    };
    let line = join.to_json_line().expect("Serialization failed");
    let parsed = ClientMessage::from_json_line(&line).expect("Deserialization failed");
    assert_eq!(join, parsed);

    let motion = ClientMessage::PointerMotion { x: 450.5, y: 320.0 };
    let motion_line = motion.to_json_line().unwrap();
    let parsed_motion = ClientMessage::from_json_line(&motion_line).unwrap();
    assert_eq!(motion, parsed_motion);

    let chat = ClientMessage::CursorChat {
        message: "Pair programming in Knot!".to_string(),
    };
    let chat_line = chat.to_json_line().unwrap();
    let parsed_chat = ClientMessage::from_json_line(&chat_line).unwrap();
    assert_eq!(chat, parsed_chat);
}

#[test]
fn test_server_message_serialization() {
    let welcome = ServerMessage::Welcome {
        session_id: 42,
        assigned_seat_name: "seat-bob".to_string(),
        user_name: "Bob".to_string(),
        user_color: "#00E5FF".to_string(),
        canvas_width: 1280,
        canvas_height: 800,
    };
    let line = welcome.to_json_line().unwrap();
    let parsed = ServerMessage::from_json_line(&line).unwrap();
    assert_eq!(welcome, parsed);
}
