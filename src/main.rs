mod input;
mod launcher;
mod net;
mod render;
mod seats;
mod state;

use std::{sync::atomic::Ordering, time::Duration};
use clap::Parser;
use smithay::{
    backend::{
        allocator::Fourcc,
        input::{AbsolutePositionEvent, ButtonState, InputEvent, KeyState, KeyboardKeyEvent, Keycode, PointerButtonEvent},
        renderer::{
            damage::OutputDamageTracker,
            gles::GlesTexture,
            glow::GlowRenderer,
            Bind, ExportMem, Offscreen,
        },
        winit::{self, WinitEvent},
    },
    output::Mode,
    reexports::{
        calloop::EventLoop,
        wayland_server::Display,
    },
    utils::{Buffer as BufferCoord, Point, Rectangle, Size, Transform},
};
use tracing::{debug, error, info, Level};
use tracing_subscriber::FmtSubscriber;
use crate::{
    input::{simulator::ConcurrentFocusSimulator, KnotInputHandler},
    net::KnotWebServer,
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

    #[arg(long, default_value = "7447", help = "Network port for multiplayer remote collaboration")]
    port: u16,
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
    let (net_server, net_rx, web_server_handle) = KnotWebServer::start(args.port);

    // Calloop Ping source to immediately wake up event loop when remote client acts
    let (ping, ping_source) = smithay::reexports::calloop::ping::make_ping()?;
    net_server.set_ping(ping);
    event_loop.handle().insert_source(ping_source, |_, _, state| {
        state.needs_redraw = true;
    })?;

    let mut offscreen_size = (1280usize, 800usize);
    // Create offscreen texture for safe pixel capture (avoids reading from Wayland EGL window surface)
    let (renderer_tmp, _) = backend.bind()?;
    let mut offscreen_texture: GlesTexture = renderer_tmp.create_buffer(
        Fourcc::Abgr8888,
        Size::from((1280i32, 800i32)),
    )?;
    info!("🖼️ [OFFSCREEN FBO] Created 1280x800 offscreen texture for safe frame capture");
    let _ = renderer_tmp; // Release the bind - we'll re-bind in the loop

    // Single-slot buffer for real-time JPEG encoding of live Wayland desktop (always encodes newest frame)
    let pending_frame = std::sync::Arc::new((
        std::sync::Mutex::new(None::<(Vec<u8>, usize, usize)>),
        std::sync::Condvar::new(),
    ));
    let pending_frame_for_encoder = pending_frame.clone();
    let web_server_for_encoder = web_server_handle.clone();
    std::thread::spawn(move || {
        use image::codecs::jpeg::JpegEncoder;
        use image::ExtendedColorType;

        let (lock, cvar) = &*pending_frame_for_encoder;
        loop {
            let (pixel_data, w, h) = {
                let mut guard = lock.lock().unwrap();
                while guard.is_none() {
                    guard = cvar.wait(guard).unwrap();
                }
                guard.take().unwrap()
            };

            // Full 1:1 native resolution with 100% maximum JPEG visual fidelity (zero downsampling)
            let out_w = w;
            let out_h = h;

            let mut rgb_pixels = vec![0u8; out_w * out_h * 3];
            let mut out_idx = 0;

            for y_out in (0..out_h).rev() {
                let row_start = y_out * w * 4;
                let src_row = &pixel_data[row_start..row_start + w * 4];
                let dst_row = &mut rgb_pixels[out_idx..out_idx + out_w * 3];
                for (dst, src) in dst_row.chunks_exact_mut(3).zip(src_row.chunks_exact(4)) {
                    dst[0] = src[0];
                    dst[1] = src[1];
                    dst[2] = src[2];
                }
                out_idx += out_w * 3;
            }

            let t_enc_start = std::time::Instant::now();
            let mut jpeg_bytes = Vec::with_capacity(256 * 1024);
            let mut encoder = JpegEncoder::new_with_quality(&mut jpeg_bytes, 100);
            if encoder.encode(&rgb_pixels, out_w as u32, out_h as u32, ExtendedColorType::Rgb8).is_ok() {
                debug!("🖼️ [JPEG ENCODE 100%] Size: {}x{} -> {} KB in {:?}", out_w, out_h, jpeg_bytes.len() / 1024, t_enc_start.elapsed());
                web_server_for_encoder.broadcast_frame(&jpeg_bytes);
            }
        }
    });

    let mut last_capture_time = std::time::Instant::now();

    // Use Transform::Flipped180 to correct OpenGL inverted Y coordinates
    state.output.change_current_state(
        Some(mode),
        Some(Transform::Flipped180),
        None,
        Some((0, 0).into()),
    );
    state.output.set_preferred(mode);
    state.space.map_output(&state.output, (0, 0));

    let mut damage_tracker = OutputDamageTracker::from_output(&state.output);
    let mut offscreen_damage_tracker = OutputDamageTracker::from_output(&state.output);
    let mut knot_renderer = KnotRenderer::new();
    // Create the display listening socket
    let socket = smithay::wayland::socket::ListeningSocketSource::with_name(&args.socket)?;
    
    event_loop.handle().insert_source(socket, |stream, _, state| {
        info!("🔗 [CLIENT CONNECTED] New client connection accepted on display socket!");
        state.display_handle.insert_client(stream, std::sync::Arc::new(KnotClientData::default())).unwrap();
    })?;

    info!("🌟 Wayland server listening on socket: WAYLAND_DISPLAY={}", args.socket);
    println!("\n=======================================================");
    println!("  🌟 KNOT MULTIPLAYER DESKTOP ACTIVE");
    println!("  🌐 Local Web Portal:    http://localhost:{}", args.port);
    println!("  🏠 Local Network (LAN): http://<YOUR-LAN-IP>:{} (e.g. http://10.17.16.205:{})", args.port, args.port);
    println!("  🚀 Share with Pinggy over Internet:");
    println!("     ssh -p 443 -R0:localhost:{} a.pinggy.io", args.port);
    println!("  • Direct Web Client: Open the generated link in ANY browser");
    println!("  • Direct Native Client: ./target/debug/knot-client --connect 127.0.0.1:{}", args.port);
    println!("=======================================================\n");

    let mut simulator = ConcurrentFocusSimulator::new();
    if args.verify_concurrent_focus {
        simulator.active = true;
        info!("🤖 Automated Concurrent Focus Verification Simulator ENABLED");
    }

    // Request initial redraw to start the presentation loop
    backend.window().request_redraw();
    let mut last_render_instant = std::time::Instant::now();

    while state.is_running {
        // Unconditionally sync connected peers count at top of loop
        state.connected_peers_count = net_server.connected_clients.load(Ordering::Relaxed);

        // 1. Poll incoming network multiplayer events
        let mut had_network_events = false;
        while let Ok(event) = net_rx.try_recv() {
            had_network_events = true;
            match event {
                net::NetworkEvent::ClientJoined { name, color: _ } => {
                    info!("🎉 [REMOTE GUEST ACTIVE] Assigned remote collaborator '{}' to seat-bob", name);
                    state.seat_manager.bob_mut().name = format!("{} (Remote)", name);
                    state.seat_manager.bob_mut().is_remote = true;
                }
                net::NetworkEvent::PointerMotion { x, y } => {
                    KnotInputHandler::handle_remote_pointer_motion(&mut state, Point::from((x, y)));
                }
                net::NetworkEvent::PointerButton { button, pressed } => {
                    KnotInputHandler::handle_remote_pointer_button(&mut state, button, pressed);
                }
                net::NetworkEvent::KeyEvent { keycode, pressed } => {
                    KnotInputHandler::handle_remote_key(&mut state, Keycode::new(keycode), pressed);
                }
                net::NetworkEvent::CursorChat { message } => {
                    state.seat_manager.bob_mut().set_cursor_chat(message);
                }
                net::NetworkEvent::ClientDisconnected => {}
            }
        }

        // 2. Dispatch Winit events (resizing, focus, local input)
        let mut winit_redraw_requested = false;
        winit_event_loop.dispatch_new_events(|event| match event {
            WinitEvent::Resized { size, .. } => {
                let mode = Mode {
                    size: Size::from((size.w as i32, size.h as i32)),
                    refresh: 60_000,
                };
                state.output.change_current_state(Some(mode), None, None, None);
            }
            WinitEvent::Focus(focused) => {
                info!("🪟 [HOST WINDOW FOCUS] Knot window focused by OS: {}", focused);
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
            }
            WinitEvent::Redraw => {
                winit_redraw_requested = true;
            }
            WinitEvent::CloseRequested => {
                state.is_running = false;
            }
        });

        // 3. Render tick: Always render when Winit requests it, when network events arrive,
        // when client buffer is committed (needs_redraw), or on a guaranteed 60 FPS clock when remote peers are connected!
        let now = std::time::Instant::now();
        let should_render = winit_redraw_requested
            || had_network_events
            || state.needs_redraw
            || (state.connected_peers_count > 0 && now.duration_since(last_render_instant) >= Duration::from_millis(16));

        if should_render {
            last_render_instant = now;
            let is_action_tick = had_network_events || state.needs_redraw;
            state.needs_redraw = false;
            simulator.tick(&mut state);

            let t_start = std::time::Instant::now();
            let mut render_ok = false;
            match backend.bind() {
                Ok((renderer, mut framebuffer)) => {
                    // 1. Always render to window framebuffer for local display
                    if let Err(err) = knot_renderer.render_frame(
                        &mut state,
                        renderer,
                        &mut framebuffer,
                        &mut damage_tracker,
                        0,
                    ) {
                        error!("Render error: {:?}", err);
                    } else {
                        render_ok = true;
                    }
                }
                Err(err) => {
                    error!("Failed to bind framebuffer: {:?}", err);
                }
            }
            let t_window_render = t_start.elapsed();

            let mut t_submit = Duration::ZERO;
            let mut t_offscreen = Duration::ZERO;
            let mut t_copy = Duration::ZERO;

            if render_ok {
                // Submit window buffer to host window while window EGLSurface is bound and current
                let t_sub_start = std::time::Instant::now();
                if let Err(err) = backend.submit(None) {
                    error!("Failed to submit buffer: {:?}", err);
                }
                t_submit = t_sub_start.elapsed();

                // Dispatch frame callbacks to unblock 60 FPS live presentation
                let time = state.start_time.elapsed().as_millis() as u32;
                for window in state.space.elements() {
                    window.send_frame(
                        &state.output,
                        Duration::from_millis(time as u64),
                        Some(Duration::ZERO),
                        |_, _| Some(state.output.clone()),
                    );
                }

                // 2. Offscreen capture for remote browser streaming:
                // Capture immediately if there's any action (network input, client buffer commit, etc.)
                // OR on regular ~16ms tick (guaranteed 60 FPS update stream)
                let should_capture = state.connected_peers_count > 0
                    && (is_action_tick || last_capture_time.elapsed() >= Duration::from_millis(16));

                if should_capture {
                    last_capture_time = std::time::Instant::now();
                    let cur_mode = state.output.current_mode().map(|m| m.size).unwrap_or_else(|| Size::from((1280, 800)));
                    let cur_w = cur_mode.w.max(640) as usize;
                    let cur_h = cur_mode.h.max(480) as usize;

                    // Reallocate offscreen texture if output size changed
                    if cur_w != offscreen_size.0 || cur_h != offscreen_size.1 {
                        if let Ok(new_tex) = backend.renderer().create_buffer(Fourcc::Abgr8888, Size::from((cur_w as i32, cur_h as i32))) {
                            offscreen_texture = new_tex;
                            offscreen_size = (cur_w, cur_h);
                            info!("🖼️ [OFFSCREEN RESIZED] Reallocated offscreen texture to {}x{}", cur_w, cur_h);
                        }
                    }

                    let t_off_start = std::time::Instant::now();
                    let renderer = backend.renderer();
                    let offscreen_rendered = {
                        if let Ok(mut offscreen_target) = renderer.bind(&mut offscreen_texture) {
                            let result = knot_renderer.render_frame(
                                &mut state,
                                renderer,
                                &mut offscreen_target,
                                &mut offscreen_damage_tracker,
                                0,
                            );
                            result.is_ok()
                        } else {
                            false
                        }
                    };
                    t_offscreen = t_off_start.elapsed();

                    if offscreen_rendered {
                        let t_cp_start = std::time::Instant::now();
                        let region = Rectangle::<i32, BufferCoord>::new(
                            (0, 0).into(),
                            (cur_w as i32, cur_h as i32).into(),
                        );
                        let renderer = backend.renderer();
                        if let Ok(mapping) = renderer.copy_texture(&offscreen_texture, region, Fourcc::Abgr8888) {
                            if let Ok(pixel_data) = renderer.map_texture(&mapping) {
                                let (lock, cvar) = &*pending_frame;
                                let mut guard = lock.lock().unwrap();
                                *guard = Some((pixel_data.to_vec(), cur_w, cur_h));
                                cvar.notify_one();
                            }
                        }
                        t_copy = t_cp_start.elapsed();
                    }
                }
            }

            if state.connected_peers_count > 0 {
                debug!(
                    "⏱️ [FRAME TIMING] Total: {:?} | WinRender: {:?} | Submit: {:?} | OffRender: {:?} | CopyMap: {:?}",
                    t_start.elapsed(),
                    t_window_render,
                    t_submit,
                    t_offscreen,
                    t_copy
                );
            }

            backend.window().request_redraw();
        }

        // Dispatch and flush Wayland display clients
        display.dispatch_clients(&mut state)?;
        display.flush_clients()?;
        event_loop.dispatch(Some(Duration::from_millis(2)), &mut state)?;
        state.space.refresh();
        state.popups.cleanup();
    }

    info!("👋 Knot Compositor shut down successfully.");
    Ok(())
}
