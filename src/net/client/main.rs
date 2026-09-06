use std::{
    io::{BufRead, BufReader, Write},
    net::TcpStream,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};
use clap::Parser;
use tracing::{error, info, Level};
use tracing_subscriber::FmtSubscriber;
use knot::net::proto::{ClientMessage, ServerMessage};

#[derive(Parser, Debug)]
#[command(author, version, about = "Knot Remote Client - Connect to a Multiplayer Desktop")]
struct ClientArgs {
    #[arg(long, default_value = "127.0.0.1:7447", help = "Host address:port")]
    connect: String,

    #[arg(long, default_value = "Bob (Remote)", help = "Your display name")]
    name: String,

    #[arg(long, default_value = "#00E5FF", help = "Cursor color in hex")]
    color: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let args = ClientArgs::parse();

    info!("🚀 [KNOT CLIENT] Connecting to host at {}...", args.connect);
    let mut stream = match TcpStream::connect(&args.connect) {
        Ok(s) => {
            info!("✅ Successfully connected to Knot Host at {}!", args.connect);
            s
        }
        Err(err) => {
            error!("❌ Could not connect to host at {}: {:?}", args.connect, err);
            return Err(err.into());
        }
    };

    let _ = stream.set_nodelay(true);

    // 1. Send Join Request
    let join_msg = ClientMessage::Join {
        user_name: args.name.clone(),
        color_hex: args.color.clone(),
    };
    stream.write_all(join_msg.to_json_line()?.as_bytes())?;
    stream.flush()?;

    // 2. Read Welcome Response
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line)?;

    if let Ok(ServerMessage::Welcome { session_id, assigned_seat_name, user_name, user_color, canvas_width, canvas_height }) = ServerMessage::from_json_line(&line) {
        println!("\n=======================================================");
        println!("  🎉 CONNECTED TO KNOT MULTIPLAYER WORKSPACE");
        println!("  • Session ID:    #{}", session_id);
        println!("  • Assigned Seat: {} ({})", assigned_seat_name, user_color);
        println!("  • User Name:     {}", user_name);
        println!("  • Canvas Size:   {}x{}", canvas_width, canvas_height);
        println!("=======================================================");
        println!("  COMMANDS AVAILABLE:");
        println!("  • Type '/chat <message>' to broadcast floating cursor chat");
        println!("  • Type '/move <x> <y>' to reposition your cursor");
        println!("  • Type '/click' to send a left mouse click");
        println!("  • Type '/launch' to open the In-Compositor App Launcher");
        println!("  • Type 'quit' to disconnect");
        println!("=======================================================\n");
    }

    let is_running = Arc::new(AtomicBool::new(true));
    let mut send_stream = stream.try_clone()?;

    // Background keep-alive / ping thread
    let running_clone = is_running.clone();
    let mut ping_stream = stream.try_clone()?;
    thread::spawn(move || {
        while running_clone.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_secs(3));
            let ping = ClientMessage::Ping;
            if let Ok(line) = ping.to_json_line() {
                let _ = ping_stream.write_all(line.as_bytes());
                let _ = ping_stream.flush();
            }
        }
    });

    // Interactive CLI Input Loop
    let stdin = std::io::stdin();
    for line_res in stdin.lock().lines() {
        let input = line_res?;
        let trimmed = input.trim();

        if trimmed == "quit" || trimmed == "exit" {
            break;
        } else if trimmed.starts_with("/chat ") {
            let msg = trimmed[6..].to_string();
            let chat_packet = ClientMessage::CursorChat { message: msg };
            send_stream.write_all(chat_packet.to_json_line()?.as_bytes())?;
            send_stream.flush()?;
            info!("💬 Sent cursor chat bubble to workspace!");
        } else if trimmed.starts_with("/move ") {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 3 {
                if let (Ok(x), Ok(y)) = (parts[1].parse::<f64>(), parts[2].parse::<f64>()) {
                    let motion = ClientMessage::PointerMotion { x, y };
                    send_stream.write_all(motion.to_json_line()?.as_bytes())?;
                    send_stream.flush()?;
                    info!("🖱️ Moved remote cursor to ({:.1}, {:.1})", x, y);
                }
            }
        } else if trimmed == "/click" {
            let press = ClientMessage::PointerButton { button: 272, pressed: true };
            let release = ClientMessage::PointerButton { button: 272, pressed: false };
            send_stream.write_all(press.to_json_line()?.as_bytes())?;
            send_stream.flush()?;
            thread::sleep(Duration::from_millis(50));
            send_stream.write_all(release.to_json_line()?.as_bytes())?;
            send_stream.flush()?;
            info!("🖱️ Clicked left mouse button on remote desktop!");
        } else if trimmed == "/launch" {
            // Super key toggle (133 in XKB)
            let press = ClientMessage::KeyEvent { keycode: 133, pressed: true };
            let release = ClientMessage::KeyEvent { keycode: 133, pressed: false };
            send_stream.write_all(press.to_json_line()?.as_bytes())?;
            send_stream.flush()?;
            thread::sleep(Duration::from_millis(50));
            send_stream.write_all(release.to_json_line()?.as_bytes())?;
            send_stream.flush()?;
            info!("🚀 Toggled In-Compositor App Launcher on host!");
        } else if trimmed.starts_with("/launch ") {
            let arg = trimmed[8..].trim();
            // First open launcher if not open
            let press = ClientMessage::KeyEvent { keycode: 133, pressed: true };
            let release = ClientMessage::KeyEvent { keycode: 133, pressed: false };
            send_stream.write_all(press.to_json_line()?.as_bytes())?;
            send_stream.flush()?;
            thread::sleep(Duration::from_millis(60));
            send_stream.write_all(release.to_json_line()?.as_bytes())?;
            send_stream.flush()?;
            thread::sleep(Duration::from_millis(60));

            if let Ok(num) = arg.parse::<u32>() {
                if (1..=7).contains(&num) {
                    let keycode = 9 + num; // 10 = '1', 11 = '2', etc.
                    let num_press = ClientMessage::KeyEvent { keycode, pressed: true };
                    let num_release = ClientMessage::KeyEvent { keycode, pressed: false };
                    send_stream.write_all(num_press.to_json_line()?.as_bytes())?;
                    send_stream.flush()?;
                    thread::sleep(Duration::from_millis(50));
                    send_stream.write_all(num_release.to_json_line()?.as_bytes())?;
                    send_stream.flush()?;
                    info!("🚀 Remotely launched App #{} via quick hotkey!", num);
                }
            } else {
                // Type search query
                for ch in arg.chars() {
                    let raw = match ch.to_ascii_lowercase() {
                        'a'..='z' => ch.to_ascii_lowercase() as u32 - b'a' as u32 + 38,
                        '0'..='9' => if ch == '0' { 19 } else { ch as u32 - b'1' as u32 + 10 },
                        _ => 65, // space
                    };
                    let p = ClientMessage::KeyEvent { keycode: raw, pressed: true };
                    let r = ClientMessage::KeyEvent { keycode: raw, pressed: false };
                    send_stream.write_all(p.to_json_line()?.as_bytes())?;
                    send_stream.flush()?;
                    thread::sleep(Duration::from_millis(20));
                    send_stream.write_all(r.to_json_line()?.as_bytes())?;
                    send_stream.flush()?;
                    thread::sleep(Duration::from_millis(20));
                }
                // Press Enter
                let enter_p = ClientMessage::KeyEvent { keycode: 36, pressed: true };
                let enter_r = ClientMessage::KeyEvent { keycode: 36, pressed: false };
                send_stream.write_all(enter_p.to_json_line()?.as_bytes())?;
                send_stream.flush()?;
                thread::sleep(Duration::from_millis(50));
                send_stream.write_all(enter_r.to_json_line()?.as_bytes())?;
                send_stream.flush()?;
                info!("🚀 Remotely searched and launched '{}'!", arg);
            }
        } else if trimmed == "/up" {
            let p = ClientMessage::KeyEvent { keycode: 111, pressed: true };
            let r = ClientMessage::KeyEvent { keycode: 111, pressed: false };
            send_stream.write_all(p.to_json_line()?.as_bytes())?;
            send_stream.flush()?;
            thread::sleep(Duration::from_millis(40));
            send_stream.write_all(r.to_json_line()?.as_bytes())?;
            send_stream.flush()?;
        } else if trimmed == "/down" {
            let p = ClientMessage::KeyEvent { keycode: 116, pressed: true };
            let r = ClientMessage::KeyEvent { keycode: 116, pressed: false };
            send_stream.write_all(p.to_json_line()?.as_bytes())?;
            send_stream.flush()?;
            thread::sleep(Duration::from_millis(40));
            send_stream.write_all(r.to_json_line()?.as_bytes())?;
            send_stream.flush()?;
        } else if trimmed == "/enter" {
            let p = ClientMessage::KeyEvent { keycode: 36, pressed: true };
            let r = ClientMessage::KeyEvent { keycode: 36, pressed: false };
            send_stream.write_all(p.to_json_line()?.as_bytes())?;
            send_stream.flush()?;
            thread::sleep(Duration::from_millis(40));
            send_stream.write_all(r.to_json_line()?.as_bytes())?;
            send_stream.flush()?;
        } else if trimmed == "/esc" {
            let p = ClientMessage::KeyEvent { keycode: 9, pressed: true };
            let r = ClientMessage::KeyEvent { keycode: 9, pressed: false };
            send_stream.write_all(p.to_json_line()?.as_bytes())?;
            send_stream.flush()?;
            thread::sleep(Duration::from_millis(40));
            send_stream.write_all(r.to_json_line()?.as_bytes())?;
            send_stream.flush()?;
        } else {
            // Forward arbitrary text as cursor chat
            let chat_packet = ClientMessage::CursorChat { message: trimmed.to_string() };
            send_stream.write_all(chat_packet.to_json_line()?.as_bytes())?;
            send_stream.flush()?;
            info!("💬 Broadcasted cursor message: '{}'", trimmed);
        }
    }

    is_running.store(false, Ordering::Relaxed);
    info!("👋 Knot Client disconnected.");
    Ok(())
}
