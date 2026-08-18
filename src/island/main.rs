use std::time::Duration;
use clap::Parser;
use smithay::reexports::calloop::EventLoop;
use smithay::reexports::wayland_server::Display;
use tracing::{debug, error, info, Level};
use tracing_subscriber::FmtSubscriber;
use wayland_client::Connection;

mod client;
mod multiplexer;
mod server;

use client::{IslandClientState, ParentEvent};
use server::IslandState;

#[derive(Parser, Debug)]
#[command(author, version, about = "Knot Island - Virtual Seat Proxy for Single-Seat Apps")]
struct Args {
    #[arg(long, default_value = "wayland-island-0", help = "Private island socket name")]
    socket: String,

    #[arg(long, default_value = "wayland-knot-0", help = "Parent compositor socket name")]
    parent_socket: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let args = Args::parse();
    info!("🏝️ Initializing Knot Island Virtual Seat Proxy on socket: {}", args.socket);

    // 1. Initialize island server loop
    let mut event_loop: EventLoop<IslandState> = EventLoop::try_new()?;
    let mut display: Display<IslandState> = Display::new()?;
    let mut state = IslandState::new(&display, event_loop.handle());

    let socket = smithay::wayland::socket::ListeningSocketSource::with_name(&args.socket)?;
    event_loop.handle().insert_source(socket, |stream, _, state| {
        info!("🔗 [ISLAND GUEST CONNECTED] Single-seat client joined isolated sandbox!");
        state.display_handle.insert_client(stream, std::sync::Arc::new(server::IslandClientData::default())).unwrap();
    })?;

    // 2. Connect as client to parent knot-core compositor
    info!("🔗 Connecting to parent Knot compositor on socket: {}", args.parent_socket);
    std::env::set_var("WAYLAND_DISPLAY", &args.parent_socket);
    
    let parent_conn = match Connection::connect_to_env() {
        Ok(c) => Some(c),
        Err(err) => {
            error!("⚠️ Could not connect to parent compositor {}: {}", args.parent_socket, err);
            None
        }
    };

    let mut client_state = IslandClientState::default();
    let client_events = client_state.event_queue.clone();
    let mut parent_event_queue = parent_conn.as_ref().map(|c| {
        let queue = c.new_event_queue();
        let qh = queue.handle();
        let display = c.display();
        display.get_registry(&qh, ());
        queue
    });
    let parent_qh = parent_event_queue.as_ref().map(|q| q.handle());

    // Synchronous initial roundtrip to discover parent globals and create parent toplevel surface
    if let (Some(ref conn), Some(ref mut queue)) = (&parent_conn, &mut parent_event_queue) {
        let _ = queue.roundtrip(&mut client_state);
        let qh = queue.handle();
        client_state.ensure_surface_created(&qh);
        let _ = conn.flush();
        let _ = queue.roundtrip(&mut client_state);
    }

    info!("🌟 Knot Island active: Listening on WAYLAND_DISPLAY={}", args.socket);

    while state.is_running {
        // Dispatch incoming parent compositor events
        if let (Some(ref conn), Some(ref mut queue)) = (&parent_conn, &mut parent_event_queue) {
            if let Some(guard) = conn.prepare_read() {
                if let Err(err) = guard.read() {
                    debug!("Parent connection disconnected: {:?}", err);
                    break;
                }
            }
            if let Err(err) = queue.dispatch_pending(&mut client_state) {
                debug!("Dispatch pending error: {:?}", err);
                break;
            }
            if let Err(err) = conn.flush() {
                debug!("Conn flush error: {:?}", err);
                break;
            }
        }

        // Process forwarded parent events and route through multiplexer
        if let Ok(mut events) = client_events.lock() {
            for event in events.drain(..) {
                match event {
                    ParentEvent::PointerMotion { seat_name, location } => {
                        state.handle_parent_pointer_motion(&seat_name, location);
                    }
                    ParentEvent::PointerButton { seat_name, button, pressed } => {
                        state.handle_parent_pointer_button(&seat_name, button, pressed);
                    }
                    ParentEvent::KeyboardKey { seat_name, keycode, pressed } => {
                        state.handle_parent_keyboard_key(&seat_name, keycode, pressed);
                    }
                }
            }
        }

        // Forward any pending buffer frames to parent knot-core
        if let (Some(ref conn), Some(ref qh)) = (&parent_conn, &parent_qh) {
            for frame in state.drain_pending_frames() {
                client_state.forward_buffer_frame(qh, frame.width, frame.height, frame.stride, &frame.bytes);
            }
            if let Err(err) = conn.flush() {
                debug!("Conn flush error on frame commit: {:?}", err);
                break;
            }
        }

        // Send frame callbacks to unblock guest application render loop!
        let time = state.start_time.elapsed();
        let output = state.output.clone();
        for window in state.space.elements() {
            window.send_frame(
                &output,
                time,
                Some(Duration::ZERO),
                |_, _| Some(output.clone()),
            );
        }

        if let Err(err) = display.dispatch_clients(&mut state) {
            debug!("Client dispatch error: {:?}", err);
            break;
        }
        if let Err(err) = display.flush_clients() {
            debug!("Client flush error: {:?}", err);
            break;
        }
        if let Err(err) = event_loop.dispatch(Some(Duration::from_millis(16)), &mut state) {
            debug!("Event loop error: {:?}", err);
            break;
        }
    }

    info!("🏝️ Knot Island shutting down cleanly.");
    Ok(())
}
