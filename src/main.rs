mod input;
mod launcher;
mod render;
mod seats;
mod state;

use std::time::Duration;
use clap::Parser;
use smithay::{
    backend::{
        input::{AbsolutePositionEvent, ButtonState, InputEvent, KeyState, KeyboardKeyEvent, Keycode, PointerButtonEvent},
        renderer::{damage::OutputDamageTracker, glow::GlowRenderer},
        winit::{self, WinitEvent},
    },
    output::Mode,
    reexports::{
        calloop::EventLoop,
        wayland_server::Display,
    },
    utils::{Point, Size, Transform},
};
use tracing::{error, info, Level};
use tracing_subscriber::FmtSubscriber;
use crate::{
    input::{simulator::ConcurrentFocusSimulator, KnotInputHandler},
    render::KnotRenderer,
    state::{KnotClientData, KnotState},
};

#[derive(Parser, Debug)]
#[command(author, version, about = "Knot - Multiplayer Wayland Desktop Compositor")]
struct Args {
    #[arg(long, help = "Run automated concurrent focus verification test")]
    verify_concurrent_focus: bool,

    #[arg(long, default_value = "wayland-knot-0", help = "Wayland socket name")]
    socket: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let args = Args::parse();

    info!("🚀 Initializing Project Knot Multiplayer Compositor...");

    let mut event_loop: EventLoop<KnotState> = EventLoop::try_new()?;
    let mut display: Display<KnotState> = Display::new()?;

    // Initialize Winit Backend with GlowRenderer
    let (mut backend, mut winit_event_loop) = winit::init::<GlowRenderer>()?;

    let mode = Mode {
        size: Size::from((1280, 800)),
        refresh: 60_000,
    };

    let mut state = KnotState::new(&display, event_loop.handle());
    state.launcher_state.parent_socket = args.socket.clone();
    // Use Transform::Flipped180 to correct OpenGL inverted Y coordinates
    state.output.change_current_state(
        Some(mode),
        Some(Transform::Flipped180),
        None,
        Some(Point::from((0, 0))),
    );
    state.space.map_output(&state.output, (0, 0));

    let mut damage_tracker = OutputDamageTracker::from_output(&state.output);
    let mut knot_renderer = KnotRenderer::new();

    // Register Wayland socket with explicit name
    let socket = match smithay::wayland::socket::ListeningSocketSource::with_name(&args.socket) {
        Ok(s) => s,
        Err(_) => smithay::wayland::socket::ListeningSocketSource::new_auto()?,
    };
    let socket_str = socket.socket_name().to_string_lossy().to_string();
    
    event_loop.handle().insert_source(socket, |stream, _, state| {
        info!("🔗 [CLIENT CONNECTED] New client connection accepted on display socket!");
        state.display_handle.insert_client(stream, std::sync::Arc::new(KnotClientData::default())).unwrap();
    })?;

    info!("🌟 Wayland server listening on socket: WAYLAND_DISPLAY={}", socket_str);
    println!("\n=======================================================");
    println!("  KNOT MULTIPLAYER DESKTOP ACTIVE");
    println!("  To open apps in Knot, run in another terminal:");
    println!("  WAYLAND_DISPLAY={} ptyxis -s &", socket_str);
    println!("  WAYLAND_DISPLAY={} code &", socket_str);
    println!("  WAYLAND_DISPLAY={} google-chrome &", socket_str);
    println!("  Hotkeys:");
    println!("  • [Tab]: Toggle between Alice (Pink 💖) and Bob (Cyan 💙)");
    println!("  • [F2]: Start/Stop Automated Concurrent Focus Test");
    println!("=======================================================\n");

    let mut simulator = ConcurrentFocusSimulator::new();
    if args.verify_concurrent_focus {
        simulator.active = true;
        info!("🤖 Automated Concurrent Focus Verification Simulator ENABLED");
    }

    // Request initial redraw to start the presentation loop
    backend.window().request_redraw();

    while state.is_running {
        // Dispatch winit events
        let mut dispatch_err = None;
        winit_event_loop.dispatch_new_events(|event| match event {
            WinitEvent::Resized { size, .. } => {
                let mode = Mode {
                    size: Size::from((size.w as i32, size.h as i32)),
                    refresh: 60_000,
                };
                state.output.change_current_state(Some(mode), None, None, None);
                backend.window().request_redraw();
            }
            WinitEvent::Focus(focused) => {
                info!("🪟 [HOST WINDOW FOCUS] Knot window focused by OS: {}", focused);
                backend.window().request_redraw();
            }
            WinitEvent::Input(input_event) => {
                match input_event {
                    InputEvent::PointerMotionAbsolute { event } => {
                        let size = state.output.current_mode().unwrap().size;
                        let point = event.position_transformed(Size::from((size.w, size.h)));
                        KnotInputHandler::handle_pointer_motion(&mut state, point);
                    }
                    InputEvent::PointerButton { event } => {
                        let button = event.button_code();
                        let pressed = event.state() == ButtonState::Pressed;
                        KnotInputHandler::handle_pointer_button(&mut state, button, pressed);
                    }
                    InputEvent::Keyboard { event } => {
                        let keycode = event.key_code();
                        let pressed = event.state() == KeyState::Pressed;

                        info!("⌨️ [RAW KEY EVENT] Keycode: {:?} | State: {:?}", keycode, event.state());

                        // Tab = Toggle User (keycode 15 or 23)
                        if pressed && (keycode == Keycode::new(15) || keycode == Keycode::new(23)) {
                            let next_user = state.seat_manager.toggle_active_user();
                            info!("🔄 [SEAT SWITCHED] Active simulator input is now: {} ({})", next_user.name, next_user.color_hex);
                        } else if pressed && (keycode == Keycode::new(60) || keycode == Keycode::new(68)) { // F2 = Toggle Automated Simulator
                            simulator.active = !simulator.active;
                            info!("🤖 Automated Concurrent Focus Simulator: {}", if simulator.active { "ACTIVE" } else { "STOPPED" });
                        } else {
                            KnotInputHandler::handle_key(&mut state, keycode, pressed);
                        }
                    }
                    _ => {}
                }
                backend.window().request_redraw();
            }
            WinitEvent::Redraw => {
                simulator.tick(&mut state);
                let mut render_ok = false;
                match backend.bind() {
                    Ok((renderer, mut framebuffer)) => {
                        let _ = knot_renderer.render_frame(&mut state, renderer, &mut framebuffer, &mut damage_tracker, 0);
                        render_ok = true;
                    }
                    Err(err) => {
                        dispatch_err = Some(err.to_string());
                    }
                }
                if render_ok {
                    let _ = backend.submit(None);

                    // Dispatch frame callbacks so clients can unblock and render animated frames
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
                }

                // Drive continuous rendering at 60 FPS
                backend.window().request_redraw();
            }
            WinitEvent::CloseRequested => {
                state.is_running = false;
            }
        });

        if let Some(err) = dispatch_err {
            error!("Render error: {}", err);
        }

        display.dispatch_clients(&mut state)?;
        display.flush_clients()?;
        event_loop.dispatch(Some(Duration::from_millis(5)), &mut state)?;
    }

    info!("Knot Compositor shutting down.");
    Ok(())
}
