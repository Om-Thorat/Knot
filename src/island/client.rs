use smithay::{
    backend::input::Keycode,
    utils::Point,
};
use std::{
    io::Write,
    os::unix::io::AsFd,
    sync::{Arc, Mutex},
};
use tempfile::tempfile;
use tracing::{debug, info, warn};
use wayland_client::{
    protocol::{
        wl_buffer::WlBuffer,
        wl_compositor::WlCompositor,
        wl_keyboard::{self, WlKeyboard},
        wl_pointer::{self, WlPointer},
        wl_registry::{self, WlRegistry},
        wl_seat::{self, WlSeat},
        wl_shm::{self, WlShm},
        wl_shm_pool::WlShmPool,
        wl_surface::WlSurface,
    },
    Connection, Dispatch, QueueHandle, WEnum,
};
use wayland_protocols::xdg::shell::client::{
    xdg_surface::{self, XdgSurface},
    xdg_toplevel::{self, XdgToplevel},
    xdg_wm_base::{self, XdgWmBase},
};

pub enum ParentEvent {
    PointerMotion {
        seat_name: String,
        location: Point<f64, smithay::utils::Logical>,
    },
    PointerButton {
        seat_name: String,
        button: u32,
        pressed: bool,
    },
    KeyboardKey {
        seat_name: String,
        keycode: Keycode,
        pressed: bool,
    },
}

#[derive(Default)]
pub struct IslandClientState {
    pub compositor: Option<WlCompositor>,
    pub shm: Option<WlShm>,
    pub wm_base: Option<XdgWmBase>,
    pub seats: Vec<(WlSeat, String)>,
    pub surface: Option<WlSurface>,
    pub xdg_surface: Option<XdgSurface>,
    pub xdg_toplevel: Option<XdgToplevel>,
    pub event_queue: Arc<Mutex<Vec<ParentEvent>>>,
    pub configured: bool,
}

impl IslandClientState {
    pub fn ensure_surface_created(&mut self, qh: &QueueHandle<Self>) {
        if self.surface.is_some() {
            return;
        }

        if let (Some(compositor), Some(wm_base)) = (&self.compositor, &self.wm_base) {
            info!("🌟 [ISLAND CLIENT] Creating corresponding XDG surface on parent knot-core compositor");
            let surface = compositor.create_surface(qh, ());
            let xdg_surface = wm_base.get_xdg_surface(&surface, qh, ());
            let xdg_toplevel = xdg_surface.get_toplevel(qh, ());
            
            xdg_toplevel.set_title("Knot Island Guest Window".to_string());
            surface.commit();

            self.surface = Some(surface);
            self.xdg_surface = Some(xdg_surface);
            self.xdg_toplevel = Some(xdg_toplevel);
        }
    }

    pub fn forward_buffer_frame(
        &mut self,
        qh: &QueueHandle<Self>,
        width: i32,
        height: i32,
        stride: i32,
        pixel_bytes: &[u8],
    ) {
        self.ensure_surface_created(qh);

        if let (Some(shm), Some(surface)) = (&self.shm, &self.surface) {
            let pool_size = pixel_bytes.len();
            if pool_size == 0 || width <= 0 || height <= 0 {
                return;
            }

            match tempfile() {
                Ok(mut file) => {
                    let _ = file.write_all(pixel_bytes);
                    let pool = shm.create_pool(file.as_fd(), pool_size as i32, qh, ());
                    let buffer = pool.create_buffer(
                        0,
                        width,
                        height,
                        stride,
                        wl_shm::Format::Argb8888,
                        qh,
                        (),
                    );

                    surface.attach(Some(&buffer), 0, 0);
                    surface.damage_buffer(0, 0, width, height);
                    surface.commit();
                    debug!("🖼️ [BUFFER FORWARDED] Frame ({}x{}) committed to parent knot-core", width, height);
                }
                Err(err) => {
                    warn!("Failed to allocate shared memory file for buffer: {:?}", err);
                }
            }
        }
    }
}

impl Dispatch<WlRegistry, ()> for IslandClientState {
    fn event(
        state: &mut Self,
        registry: &WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_registry::Event::Global { name, interface, version } => {
                debug!("🌐 [PARENT GLOBAL] #{}: {} v{}", name, interface, version);
                match interface.as_str() {
                    "wl_compositor" => {
                        state.compositor = Some(registry.bind::<WlCompositor, _, _>(name, 4, qh, ()));
                        state.ensure_surface_created(qh);
                    }
                    "wl_shm" => {
                        state.shm = Some(registry.bind::<WlShm, _, _>(name, 1, qh, ()));
                    }
                    "xdg_wm_base" => {
                        state.wm_base = Some(registry.bind::<XdgWmBase, _, _>(name, 1, qh, ()));
                        state.ensure_surface_created(qh);
                    }
                    "wl_seat" => {
                        let seat = registry.bind::<WlSeat, _, _>(name, 5, qh, ());
                        state.seats.push((seat, format!("seat-{}", name)));
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<WlCompositor, ()> for IslandClientState {
    fn event(_: &mut Self, _: &WlCompositor, _: wayland_client::protocol::wl_compositor::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<WlShm, ()> for IslandClientState {
    fn event(_: &mut Self, _: &WlShm, _: wayland_client::protocol::wl_shm::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<WlShmPool, ()> for IslandClientState {
    fn event(_: &mut Self, _: &WlShmPool, _: wayland_client::protocol::wl_shm_pool::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<WlBuffer, ()> for IslandClientState {
    fn event(_: &mut Self, buffer: &WlBuffer, event: wayland_client::protocol::wl_buffer::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        if let wayland_client::protocol::wl_buffer::Event::Release = event {
            buffer.destroy();
        }
    }
}

impl Dispatch<XdgWmBase, ()> for IslandClientState {
    fn event(_: &mut Self, wm_base: &XdgWmBase, event: xdg_wm_base::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        if let xdg_wm_base::Event::Ping { serial } = event {
            wm_base.pong(serial);
        }
    }
}

impl Dispatch<WlSeat, ()> for IslandClientState {
    fn event(
        state: &mut Self,
        seat: &WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_seat::Event::Capabilities { capabilities } => {
                if let WEnum::Value(caps) = capabilities {
                    if caps.contains(wl_seat::Capability::Pointer) {
                        let _ = seat.get_pointer(qh, ());
                    }
                    if caps.contains(wl_seat::Capability::Keyboard) {
                        let _ = seat.get_keyboard(qh, ());
                    }
                }
            }
            wl_seat::Event::Name { name } => {
                info!("🪑 [PARENT SEAT DISCOVERED] Binding multi-user seat: '{}'", name);
                if let Some(entry) = state.seats.iter_mut().find(|(s, _)| s == seat) {
                    entry.1 = name;
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<WlPointer, ()> for IslandClientState {
    fn event(
        state: &mut Self,
        _pointer: &WlPointer,
        event: wl_pointer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let queue = state.event_queue.clone();
        match event {
            wl_pointer::Event::Motion { surface_x, surface_y, .. } => {
                let loc = Point::from((surface_x, surface_y));
                if let Ok(mut q) = queue.lock() {
                    q.push(ParentEvent::PointerMotion {
                        seat_name: "seat-parent".to_string(),
                        location: loc,
                    });
                }
            }
            wl_pointer::Event::Button { button, state: b_state, .. } => {
                let pressed = b_state == WEnum::Value(wl_pointer::ButtonState::Pressed);
                if let Ok(mut q) = queue.lock() {
                    q.push(ParentEvent::PointerButton {
                        seat_name: "seat-parent".to_string(),
                        button,
                        pressed,
                    });
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<WlKeyboard, ()> for IslandClientState {
    fn event(
        state: &mut Self,
        _keyboard: &WlKeyboard,
        event: wl_keyboard::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let queue = state.event_queue.clone();
        if let wl_keyboard::Event::Key { key, state: k_state, .. } = event {
            let pressed = k_state == WEnum::Value(wl_keyboard::KeyState::Pressed);
            if let Ok(mut q) = queue.lock() {
                // Wayland wire format delivers raw linux evdev code (e.g. Backspace = 14).
                // Smithay's keyboard engine internally operates in XKB keycode space (evdev + 8),
                // and subtracts 8 when emitting to clients.
                let xkb_keycode = Keycode::new(key + 8);
                q.push(ParentEvent::KeyboardKey {
                    seat_name: "seat-parent".to_string(),
                    keycode: xkb_keycode,
                    pressed,
                });
            }
        }
    }
}

impl Dispatch<WlSurface, ()> for IslandClientState {
    fn event(_: &mut Self, _: &WlSurface, _: wayland_client::protocol::wl_surface::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<XdgSurface, ()> for IslandClientState {
    fn event(state: &mut Self, xdg_surface: &XdgSurface, event: xdg_surface::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        if let xdg_surface::Event::Configure { serial } = event {
            info!("🎉 [PARENT CONFIGURE RECEIVED] Acking configure serial: {}", serial);
            xdg_surface.ack_configure(serial);
            if let Some(surface) = &state.surface {
                surface.commit();
            }
            state.configured = true;
        }
    }
}

impl Dispatch<XdgToplevel, ()> for IslandClientState {
    fn event(_: &mut Self, _: &XdgToplevel, _: xdg_toplevel::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
