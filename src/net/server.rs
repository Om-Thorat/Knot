use std::{
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::{self, Receiver, Sender},
        Arc,
    },
    thread,
    time::Duration,
};
use tracing::{debug, error, info, warn};
use super::proto::{ClientMessage, ServerMessage};

#[derive(Debug, Clone)]
pub enum NetworkEvent {
    ClientJoined { name: String, color: String },
    PointerMotion { x: f64, y: f64 },
    PointerButton { button: u32, pressed: bool },
    KeyEvent { keycode: u32, pressed: bool },
    CursorChat { message: String },
    ClientDisconnected,
}

pub struct KnotNetworkServer {
    pub port: u16,
    pub connected_clients: Arc<AtomicUsize>,
    is_running: Arc<AtomicBool>,
}

impl KnotNetworkServer {
    pub fn start(port: u16) -> (Self, Receiver<NetworkEvent>) {
        let (tx, rx) = mpsc::channel();
        let connected_clients = Arc::new(AtomicUsize::new(0));
        let is_running = Arc::new(AtomicBool::new(true));

        let connected_clone = connected_clients.clone();
        let running_clone = is_running.clone();

        thread::spawn(move || {
            let addr = format!("0.0.0.0:{}", port);
            let listener = match TcpListener::bind(&addr) {
                Ok(l) => {
                    info!("🌐 [KNOT NETWORK SERVER] Multiplayer Host listening on {}", addr);
                    l
                }
                Err(err) => {
                    error!("❌ [KNOT NETWORK SERVER] Failed to bind port {}: {:?}", port, err);
                    return;
                }
            };

            // Non-blocking accept loop with small sleep
            listener.set_nonblocking(true).unwrap_or(());

            while running_clone.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, peer_addr)) => {
                        info!("✨ [REMOTE PEER CONNECTED] Incoming connection from {}", peer_addr);
                        let client_tx = tx.clone();
                        let client_count = connected_clone.clone();
                        let client_running = running_clone.clone();

                        thread::spawn(move || {
                            Self::handle_client(stream, client_tx, client_count, client_running);
                        });
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(15));
                    }
                    Err(err) => {
                        warn!("Network accept error: {:?}", err);
                        thread::sleep(Duration::from_millis(50));
                    }
                }
            }
        });

        (
            Self {
                port,
                connected_clients,
                is_running,
            },
            rx,
        )
    }

    pub fn handle_client(
        mut stream: TcpStream,
        tx: Sender<NetworkEvent>,
        connected_clients: Arc<AtomicUsize>,
        is_running: Arc<AtomicBool>,
    ) {
        connected_clients.fetch_add(1, Ordering::SeqCst);
        let _ = stream.set_nodelay(true);
        let mut reader = BufReader::new(stream.try_clone().unwrap_or_else(|_| stream.try_clone().unwrap()));

        let mut line = String::new();
        while is_running.load(Ordering::Relaxed) {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => break, // EOF / Client disconnected
                Ok(_) => {
                    if let Ok(msg) = ClientMessage::from_json_line(&line) {
                        match msg {
                            ClientMessage::Join { user_name, color_hex } => {
                                info!("🤝 [GUEST JOINED] {} ({}) joined session!", user_name, color_hex);
                                let welcome = ServerMessage::Welcome {
                                    session_id: 1001,
                                    assigned_seat_name: "seat-bob".to_string(),
                                    user_name: user_name.clone(),
                                    user_color: "#00E5FF".to_string(),
                                    canvas_width: 1280,
                                    canvas_height: 800,
                                };
                                if let Ok(resp) = welcome.to_json_line() {
                                    let _ = stream.write_all(resp.as_bytes());
                                    let _ = stream.flush();
                                }
                                let _ = tx.send(NetworkEvent::ClientJoined { name: user_name, color: color_hex });
                            }
                            ClientMessage::PointerMotion { x, y } => {
                                let _ = tx.send(NetworkEvent::PointerMotion { x, y });
                            }
                            ClientMessage::PointerButton { button, pressed } => {
                                let _ = tx.send(NetworkEvent::PointerButton { button, pressed });
                            }
                            ClientMessage::KeyEvent { keycode, pressed } => {
                                let _ = tx.send(NetworkEvent::KeyEvent { keycode, pressed });
                            }
                            ClientMessage::CursorChat { message } => {
                                info!("💬 [REMOTE CURSOR CHAT] '{}'", message);
                                let _ = tx.send(NetworkEvent::CursorChat { message });
                            }
                            ClientMessage::Emote { emoji } => {
                                let _ = tx.send(NetworkEvent::CursorChat { message: emoji });
                            }
                            ClientMessage::Ping => {
                                if let Ok(resp) = ServerMessage::Pong.to_json_line() {
                                    let _ = stream.write_all(resp.as_bytes());
                                    let _ = stream.flush();
                                }
                            }
                        }
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(5));
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::ConnectionReset || e.kind() == std::io::ErrorKind::BrokenPipe || e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    break;
                }
                Err(err) => {
                    debug!("Client disconnect: {:?}", err);
                    break;
                }
            }
        }

        connected_clients.fetch_sub(1, Ordering::SeqCst);
        let _ = tx.send(NetworkEvent::ClientDisconnected);
        info!("🔌 [REMOTE PEER DISCONNECTED] Cleaned up remote session.");
    }
}
