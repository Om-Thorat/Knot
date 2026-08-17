use smithay::{
    backend::renderer::utils::on_commit_buffer_handler,
    delegate_compositor, delegate_data_device, delegate_output, delegate_seat, delegate_shm, delegate_xdg_decoration, delegate_xdg_shell,
    desktop::{PopupKind, PopupManager, Space, Window},
    input::{
        Seat, SeatHandler, SeatState,
    },
    output::Output,
    reexports::{
        calloop::LoopHandle,
        wayland_server::{
            backend::ClientData,
            protocol::{wl_buffer::WlBuffer, wl_surface::WlSurface},
            Display, DisplayHandle, Resource,
        },
    },
    utils::{Logical, Point, Serial},
    wayland::{
        buffer::BufferHandler,
        compositor::{CompositorClientState, CompositorHandler, CompositorState},
        output::OutputHandler,
        selection::{
            data_device::{
                ClientDndGrabHandler, DataDeviceHandler, DataDeviceState, ServerDndGrabHandler,
            },
            SelectionHandler,
        },
        shell::xdg::{
            decoration::{XdgDecorationHandler, XdgDecorationState},
            PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
        },
        shm::{ShmHandler, ShmState},
    },
};
use std::time::Instant;
use tracing::{info, warn};
use crate::seats::KnotSeatManager;

#[derive(Default)]
pub struct KnotClientData {
    pub compositor_state: CompositorClientState,
}
impl ClientData for KnotClientData {}

pub struct KnotState {
    pub display_handle: DisplayHandle,
    pub space: Space<Window>,
    pub popups: PopupManager,
    pub output: Output,
    pub compositor_state: CompositorState,
    pub xdg_shell_state: XdgShellState,
    pub xdg_decoration_state: XdgDecorationState,
    pub data_device_state: DataDeviceState,
    pub shm_state: ShmState,
    pub seat_state: SeatState<KnotState>,
    pub seat_manager: KnotSeatManager,
    pub drag_state: Option<(Window, Point<f64, Logical>)>,
    pub start_time: Instant,
    pub is_running: bool,
}

impl KnotState {
    pub fn new(display: &Display<KnotState>, _loop_handle: LoopHandle<'static, KnotState>) -> Self {
        let dh = display.handle();

        let compositor_state = CompositorState::new::<KnotState>(&dh);
        let xdg_shell_state = XdgShellState::new::<KnotState>(&dh);
        let xdg_decoration_state = XdgDecorationState::new::<KnotState>(&dh);
        let data_device_state = DataDeviceState::new::<KnotState>(&dh);
        let shm_state = ShmState::new::<KnotState>(&dh, vec![]);
        let mut seat_state = SeatState::new();

        let seat_manager = KnotSeatManager::new(&dh, &mut seat_state);

        let space = Space::default();
        let popups = PopupManager::default();

        let output = Output::new(
            "knot-display-0".to_string(),
            smithay::output::PhysicalProperties {
                size: (1280, 800).into(),
                subpixel: smithay::output::Subpixel::Unknown,
                make: "Knot".to_string(),
                model: "Spatial Display".to_string(),
            },
        );

        Self {
            display_handle: dh,
            space,
            popups,
            output,
            compositor_state,
            xdg_shell_state,
            xdg_decoration_state,
            data_device_state,
            shm_state,
            seat_state,
            seat_manager,
            drag_state: None,
            start_time: Instant::now(),
            is_running: true,
        }
    }
}

// Delegate macros for Smithay
delegate_compositor!(KnotState);
delegate_shm!(KnotState);
delegate_seat!(KnotState);
delegate_xdg_shell!(KnotState);
delegate_xdg_decoration!(KnotState);
delegate_data_device!(KnotState);
delegate_output!(KnotState);

impl CompositorHandler for KnotState {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }

    fn client_compositor_state<'a>(&self, client: &'a smithay::reexports::wayland_server::Client) -> &'a CompositorClientState {
        &client.get_data::<KnotClientData>().unwrap().compositor_state
    }

    fn commit(&mut self, surface: &WlSurface) {
        on_commit_buffer_handler::<Self>(surface);
        self.popups.commit(surface);
        self.space.elements().for_each(|window| window.on_commit());
    }
}

impl BufferHandler for KnotState {
    fn buffer_destroyed(&mut self, _buffer: &WlBuffer) {}
}

impl ShmHandler for KnotState {
    fn shm_state(&self) -> &ShmState {
        &self.shm_state
    }
}

impl SeatHandler for KnotState {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<KnotState> {
        &mut self.seat_state
    }

    fn focus_changed(&mut self, seat: &Seat<Self>, focused: Option<&WlSurface>) {
        info!("🔔 [SEAT FOCUS EVENT] Seat: '{}' -> Focused Surface: {:?}", seat.name(), focused);
    }

    fn cursor_image(&mut self, seat: &Seat<Self>, image: smithay::input::pointer::CursorImageStatus) {
        info!("🎨 [CURSOR IMAGE CHANGED] Seat: '{}' -> Image: {:?}", seat.name(), image);
    }
}

impl XdgShellHandler for KnotState {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        let window = Window::new_wayland_window(surface.clone());
        info!("🎉 [NEW XDG TOPLEVEL] Created Window for surface: {:?}", surface.wl_surface().id());

        // Configure initial size
        surface.with_pending_state(|state| {
            state.size = Some((560, 360).into());
        });
        surface.send_configure();

        // Tile windows side-by-side
        let count = self.space.elements().count();
        let x_pos = if count == 0 { 50 } else { (count * 580) % 700 + 50 };
        let y_pos = if count == 0 { 60 } else { ((count * 100) % 250) + 60 };

        info!("📌 [MAPPING WINDOW] Space Element mapped at ({}, {})", x_pos, y_pos);
        self.space.map_element(window, (x_pos as i32, y_pos as i32), true);
    }

    fn new_popup(&mut self, surface: PopupSurface, _positioner: PositionerState) {
        if let Err(err) = self.popups.track_popup(PopupKind::Xdg(surface)) {
            warn!("Failed to track popup: {:?}", err);
        }
    }

    fn move_request(
        &mut self,
        surface: ToplevelSurface,
        _seat: smithay::reexports::wayland_server::protocol::wl_seat::WlSeat,
        _serial: Serial,
    ) {
        let user_loc = self.seat_manager.current_user().location;
        if let Some(window) = self.space.elements().find(|w| w.toplevel().map(|t| t == &surface).unwrap_or(false)).cloned() {
            if let Some(win_loc) = self.space.element_location(&window) {
                info!("✋ [NATIVE MOVE REQUEST] Dragging window from header bar");
                self.drag_state = Some((window, Point::from((user_loc.x - win_loc.x as f64, user_loc.y - win_loc.y as f64))));
            }
        }
    }

    fn grab(&mut self, _surface: PopupSurface, _seat: smithay::reexports::wayland_server::protocol::wl_seat::WlSeat, _serial: Serial) {}
    fn reposition_request(&mut self, _surface: PopupSurface, _positioner: PositionerState, _token: u32) {}
}

impl XdgDecorationHandler for KnotState {
    fn new_decoration(&mut self, toplevel: ToplevelSurface) {
        use smithay::reexports::wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode;
        toplevel.with_pending_state(|state| {
            state.decoration_mode = Some(Mode::ClientSide);
        });
        toplevel.send_configure();
    }

    fn request_mode(&mut self, toplevel: ToplevelSurface, mode: smithay::reexports::wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode) {
        toplevel.with_pending_state(|state| {
            state.decoration_mode = Some(mode);
        });
        toplevel.send_configure();
    }

    fn unset_mode(&mut self, toplevel: ToplevelSurface) {
        use smithay::reexports::wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode;
        toplevel.with_pending_state(|state| {
            state.decoration_mode = Some(Mode::ClientSide);
        });
        toplevel.send_configure();
    }
}

impl SelectionHandler for KnotState {
    type SelectionUserData = ();
}

impl DataDeviceHandler for KnotState {
    fn data_device_state(&self) -> &DataDeviceState {
        &self.data_device_state
    }
}

impl ClientDndGrabHandler for KnotState {}
impl ServerDndGrabHandler for KnotState {}
impl OutputHandler for KnotState {}
