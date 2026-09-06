use std::{
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};
use smithay::reexports::calloop::ping::Ping;
use tracing::{debug, error, info};
use tungstenite::{accept_hdr, handshake::server::{Request, Response}, Message};
use super::server::NetworkEvent;

pub const WEB_CLIENT_HTML: &str = include_str!("web_client.html");

pub struct KnotWebServer {
    pub port: u16,
    pub connected_clients: Arc<AtomicUsize>,
    latest_frame: Arc<Mutex<Option<(u64, Arc<Vec<u8>>)>>>,
    latest_text: Arc<Mutex<Option<Arc<String>>>>,
    pub ping: Arc<Mutex<Option<Ping>>>,
    is_running: Arc<AtomicBool>,
}

impl KnotWebServer {
    pub fn set_ping(&self, ping: Ping) {
        if let Ok(mut slot) = self.ping.lock() {
            *slot = Some(ping);
        }
    }

    pub fn start(port: u16) -> (Self, Receiver<NetworkEvent>, Arc<Self>) {
        let (tx, rx) = mpsc::channel();
        let connected_clients = Arc::new(AtomicUsize::new(0));
        let is_running = Arc::new(AtomicBool::new(true));
        let latest_frame = Arc::new(Mutex::new(None));
        let latest_text = Arc::new(Mutex::new(None));
        let ping = Arc::new(Mutex::new(None));

        let server = Arc::new(Self {
            port,
            connected_clients: connected_clients.clone(),
            latest_frame: latest_frame.clone(),
            latest_text: latest_text.clone(),
            ping: ping.clone(),
            is_running: is_running.clone(),
        });

        let server_clone = server.clone();
        let connected_clone = connected_clients.clone();
        let running_clone = is_running.clone();
        let frame_clone = latest_frame.clone();
        let text_clone = latest_text.clone();
        let ping_clone = ping.clone();

        thread::spawn(move || {
            let addr = format!("0.0.0.0:{}", port);
            let listener = match TcpListener::bind(&addr) {
                Ok(l) => {
                    info!("🌐 [KNOT WEB & MULTIPLAYER SERVER] Listening on http://0.0.0.0:{}", port);
                    l
                }
                Err(err) => {
                    error!("❌ [KNOT WEB SERVER] Failed to bind port {}: {:?}", port, err);
                    return;
                }
            };

            listener.set_nonblocking(true).unwrap_or(());

            while running_clone.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, peer_addr)) => {
                        info!("✨ [INCOMING CONNECTION] Remote client from {}", peer_addr);
                        let client_tx = tx.clone();
                        let client_count = connected_clone.clone();
                        let client_running = running_clone.clone();
                        let client_frame = frame_clone.clone();
                        let client_text = text_clone.clone();
                        let client_ping = ping_clone.clone();

                        thread::spawn(move || {
                            Self::handle_connection(stream, client_tx, client_count, client_running, client_frame, client_text, client_ping);
                        });
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(err) => {
                        debug!("Accept note: {:?}", err);
                        thread::sleep(Duration::from_millis(50));
                    }
                }
            }
        });

        (
            Self {
                port,
                connected_clients,
                latest_frame,
                latest_text,
                ping,
                is_running,
            },
            rx,
            server_clone,
        )
    }

    fn handle_connection(
        mut stream: TcpStream,
        tx: Sender<NetworkEvent>,
        connected_clients: Arc<AtomicUsize>,
        is_running: Arc<AtomicBool>,
        latest_frame: Arc<Mutex<Option<(u64, Arc<Vec<u8>>)>>>,
        _latest_text: Arc<Mutex<Option<Arc<String>>>>,
        ping: Arc<Mutex<Option<Ping>>>,
    ) {
        let _ = stream.set_nodelay(true);

        // Peek at incoming request to determine if it's HTTP GET / or WebSocket upgrade
        let mut peek_buf = [0u8; 1024];
        let n = match stream.peek(&mut peek_buf) {
            Ok(n) if n > 0 => n,
            _ => return,
        };
        let req_header = String::from_utf8_lossy(&peek_buf[..n]);

        // 1. If HTTP GET / HEAD / -> Serve Web Client HTML5 app
        if req_header.starts_with("GET / ") || req_header.starts_with("GET /index.html") || req_header.starts_with("GET /?")
            || req_header.starts_with("HEAD / ") || req_header.starts_with("HEAD /index.html") {
            let mut reader = BufReader::new(&stream);
            let mut line = String::new();
            while let Ok(len) = reader.read_line(&mut line) {
                if len == 0 || line.trim().is_empty() {
                    break;
                }
                line.clear();
            }

            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\nAccess-Control-Allow-Origin: *\r\n\r\n{}",
                WEB_CLIENT_HTML.len(),
                WEB_CLIENT_HTML
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
            info!("📄 [HTTP SERVED] In-Browser Collaborative Web Portal served to client!");
            return;
        }

        // 2. If WebSocket Upgrade (/ws or Upgrade: websocket)
        if req_header.contains("Upgrade: websocket") || req_header.contains("upgrade: websocket") {
            let callback = |_req: &Request, response: Response| Ok(response);
            let mut websocket = match accept_hdr(stream, callback) {
                Ok(ws) => ws,
                Err(err) => {
                    debug!("WebSocket handshake failed: {:?}", err);
                    return;
                }
            };

            info!("🎉 [WEBSOCKET CONNECTED] Remote Browser Session upgraded successfully!");
            connected_clients.fetch_add(1, Ordering::SeqCst);
            if let Ok(guard) = ping.lock() {
                if let Some(ref p) = *guard {
                    p.ping();
                }
            }

            let _ = websocket.get_mut().set_nonblocking(true);
            let mut last_sent_frame_id = 0u64;
            let mut pending_flush = false;
            let mut client_ready_for_frame = true;
            let mut last_frame_sent_time = Instant::now() - Duration::from_millis(1000);

            while is_running.load(Ordering::Relaxed) {
                // 1. If data is still flushing to the socket (e.g. over Pinggy/WAN tunnel), drain it without queueing new frames
                if pending_flush {
                    match websocket.flush() {
                        Ok(()) => {
                            pending_flush = false;
                        }
                        Err(tungstenite::Error::Io(ref e)) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            // Socket buffer still draining; do not push new messages yet
                        }
                        Err(err) => {
                            debug!("WS flush failed: {:?}", err);
                            break;
                        }
                    }
                }

                // 2. Send newest binary frame ONLY if write buffer is drained AND client is ready (or 250ms fallback timeout)
                if !pending_flush {
                    let current_frame = {
                        if let Ok(frame_opt) = latest_frame.lock() {
                            frame_opt.clone()
                        } else {
                            None
                        }
                    };

                    if let Some((frame_id, frame)) = current_frame {
                        let timed_out = last_frame_sent_time.elapsed() >= Duration::from_millis(250);
                        if frame_id > last_sent_frame_id && (client_ready_for_frame || timed_out) {
                            client_ready_for_frame = false;
                            last_sent_frame_id = frame_id;
                            last_frame_sent_time = Instant::now();

                            let msg = Message::Binary(frame.as_ref().clone());
                            match websocket.send(msg) {
                                Ok(()) => {
                                    pending_flush = false;
                                }
                                Err(tungstenite::Error::Io(ref e)) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                    // Queued in tungstenite write buffer; will drain on subsequent flush calls
                                    pending_flush = true;
                                }
                                Err(err) => {
                                    debug!("WS frame send failed: {:?}", err);
                                    break;
                                }
                            }
                        }
                    }
                }

                // 3. Drain all available incoming input events from browser
                let mut had_input = false;
                loop {
                    match websocket.read() {
                        Ok(Message::Text(text)) => {
                            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                                if let Some(msg_type) = val.get("type").and_then(|t| t.as_str()) {
                                    had_input = true;
                                    match msg_type {
                                        "join" => {
                                            client_ready_for_frame = true;
                                            let name = val.get("user_name").and_then(|n| n.as_str()).unwrap_or("Bob (Web)").to_string();
                                            let color = val.get("color_hex").and_then(|c| c.as_str()).unwrap_or("#00E5FF").to_string();
                                            info!("🤝 [WEB GUEST JOINED] {} ({}) joined session!", name, color);
                                            let _ = tx.send(NetworkEvent::ClientJoined { name, color });
                                        }
                                        "frame_ack" => {
                                            client_ready_for_frame = true;
                                        }
                                        "pointer_motion" => {
                                            if let (Some(x), Some(y)) = (val.get("x").and_then(|v| v.as_f64()), val.get("y").and_then(|v| v.as_f64())) {
                                                let _ = tx.send(NetworkEvent::PointerMotion { x, y });
                                            }
                                        }
                                        "pointer_button" => {
                                            if let (Some(button), Some(pressed)) = (val.get("button").and_then(|v| v.as_u64()), val.get("pressed").and_then(|v| v.as_bool())) {
                                                let _ = tx.send(NetworkEvent::PointerButton { button: button as u32, pressed });
                                            }
                                        }
                                        "key_event" => {
                                            if let (Some(keycode), Some(pressed)) = (val.get("keycode").and_then(|v| v.as_u64()), val.get("pressed").and_then(|v| v.as_bool())) {
                                                let _ = tx.send(NetworkEvent::KeyEvent { keycode: keycode as u32, pressed });
                                            }
                                        }
                                        "cursor_chat" => {
                                            if let Some(msg) = val.get("message").and_then(|m| m.as_str()) {
                                                info!("💬 [WEB CURSOR CHAT] '{}'", msg);
                                                let _ = tx.send(NetworkEvent::CursorChat { message: msg.to_string() });
                                            }
                                        }
                                        "ping" => {
                                            let pong = serde_json::json!({ "type": "pong" }).to_string();
                                            let _ = websocket.send(Message::Text(pong));
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                        Ok(Message::Close(_)) => break,
                        Err(tungstenite::Error::Io(ref e)) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            break;
                        }
                        Err(err) => {
                            debug!("WS read note: {:?}", err);
                            break;
                        }
                        _ => break,
                    }
                }

                if had_input {
                    if let Ok(guard) = ping.lock() {
                        if let Some(ref p) = *guard {
                            p.ping();
                        }
                    }
                }

                thread::sleep(Duration::from_millis(5));
            }

            connected_clients.fetch_sub(1, Ordering::SeqCst);
            let _ = tx.send(NetworkEvent::ClientDisconnected);
            if let Ok(guard) = ping.lock() {
                if let Some(ref p) = *guard {
                    p.ping();
                }
            }
            info!("🔌 [WEBSOCKET DISCONNECTED] Remote Browser Session cleaned up.");
            return;
        }

        // 3. Fallback to CLI / native knot-client JSON Lines protocol
        super::server::KnotNetworkServer::handle_client(stream, tx, connected_clients, is_running);
    }

    pub fn broadcast_frame(&self, jpeg_data: &[u8]) {
        if let Ok(mut frame_slot) = self.latest_frame.lock() {
            let next_id = frame_slot.as_ref().map(|(id, _)| id.wrapping_add(1)).unwrap_or(1);
            *frame_slot = Some((next_id, Arc::new(jpeg_data.to_vec())));
        }
    }

    pub fn broadcast_text(&self, text: &str) {
        if let Ok(mut text_slot) = self.latest_text.lock() {
            *text_slot = Some(Arc::new(text.to_string()));
        }
    }
}
