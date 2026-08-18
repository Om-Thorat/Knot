use smithay::{
    backend::renderer::utils::{on_commit_buffer_handler, with_renderer_surface_state},
    delegate_compositor, delegate_data_device, delegate_output, delegate_seat, delegate_shm, delegate_xdg_decoration, delegate_xdg_shell,
    desktop::{PopupKind, PopupManager, Space, Window},
    input::{
        Seat, SeatHandler, SeatState,
    },
    output::{Mode, Output, Scale},
    reexports::{
        calloop::LoopHandle,
        wayland_server::{
            backend::{ClientData, GlobalId},
            protocol::{wl_buffer::WlBuffer, wl_surface::WlSurface},
            Display, DisplayHandle, Resource,
        },
    },
    utils::{Logical, Point, Serial, Size, Transform},
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
        shm::{with_buffer_contents, ShmHandler, ShmState},
    },
};
use std::time::Instant;
use tracing::{debug, info, warn};
use super::multiplexer::SeatMultiplexer;

#[derive(Default)]
pub struct IslandClientData {
    pub compositor_state: CompositorClientState,
}
impl ClientData for IslandClientData {}

pub struct CommittedFrame {
    pub width: i32,
    pub height: i32,
    pub stride: i32,
    pub bytes: Vec<u8>,
}

pub struct IslandState {
    pub display_handle: DisplayHandle,
    pub space: Space<Window>,
    pub popups: PopupManager,
    pub output: Output,
    pub _output_global: GlobalId,
    pub compositor_state: CompositorState,
    pub xdg_shell_state: XdgShellState,
    pub xdg_decoration_state: XdgDecorationState,
    pub data_device_state: DataDeviceState,
    pub shm_state: ShmState,
    pub seat_state: SeatState<IslandState>,
    pub multiplexer: SeatMultiplexer,
    pub pending_frames: Vec<CommittedFrame>,
    pub start_time: Instant,
    pub is_running: bool,
}

impl IslandState {
    pub fn new(display: &Display<IslandState>, _loop_handle: LoopHandle<'static, IslandState>) -> Self {
        let dh = display.handle();

        let compositor_state = CompositorState::new::<IslandState>(&dh);
        let xdg_shell_state = XdgShellState::new::<IslandState>(&dh);
        let xdg_decoration_state = XdgDecorationState::new::<IslandState>(&dh);
        let data_device_state = DataDeviceState::new::<IslandState>(&dh);
        let shm_state = ShmState::new::<IslandState>(&dh, vec![]);
        let mut seat_state = SeatState::new();

        let mut multiplexer = SeatMultiplexer::new();
        let mut virtual_seat = seat_state.new_wl_seat(&dh, "default-seat");
        let _ = virtual_seat.add_pointer();
        let _ = virtual_seat.add_keyboard(Default::default(), 200, 25);
        multiplexer.set_virtual_seat(virtual_seat);

        let space = Space::default();
        let popups = PopupManager::default();

        let output = Output::new(
            "island-display-0".to_string(),
            smithay::output::PhysicalProperties {
                size: (1920, 1080).into(),
                subpixel: smithay::output::Subpixel::Unknown,
                make: "Knot".to_string(),
                model: "Island Sandbox Display".to_string(),
            },
        );

        let mode = Mode {
            size: Size::from((1920, 1080)),
            refresh: 60_000,
        };
        output.change_current_state(Some(mode), Some(Transform::Normal), Some(Scale::Integer(1)), Some(Point::from((0, 0))));
        output.set_preferred(mode);
        let output_global = output.create_global::<IslandState>(&dh);

        Self {
            display_handle: dh,
            space,
            popups,
            output,
            _output_global: output_global,
            compositor_state,
            xdg_shell_state,
            xdg_decoration_state,
            data_device_state,
            shm_state,
            seat_state,
            multiplexer,
            pending_frames: Vec::new(),
            start_time: Instant::now(),
            is_running: true,
        }
    }

    pub fn get_surface_under(&self, location: Point<f64, Logical>) -> Option<(WlSurface, Point<f64, Logical>)> {
        for window in self.space.elements() {
            if let Some(win_loc) = self.space.element_location(window) {
                let rel_point = location - win_loc.to_f64();
                if let Some((surface, surf_offset)) = window.surface_under(rel_point, smithay::desktop::WindowSurfaceType::ALL) {
                    let surface_global_origin = win_loc.to_f64() + surf_offset.to_f64();
                    return Some((surface, surface_global_origin));
                }
            }
        }
        None
    }

    pub fn get_root_surface(&self) -> Option<WlSurface> {
        self.space.elements().next().and_then(|w| w.toplevel().map(|t| t.wl_surface().clone()))
    }

    pub fn drain_pending_frames(&mut self) -> Vec<CommittedFrame> {
        std::mem::take(&mut self.pending_frames)
    }
}

// Delegate macros for Smithay
delegate_compositor!(IslandState);
delegate_shm!(IslandState);
delegate_seat!(IslandState);
delegate_xdg_shell!(IslandState);
delegate_xdg_decoration!(IslandState);
delegate_data_device!(IslandState);
delegate_output!(IslandState);

impl CompositorHandler for IslandState {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }

    fn client_compositor_state<'a>(&self, client: &'a smithay::reexports::wayland_server::Client) -> &'a CompositorClientState {
        &client.get_data::<IslandClientData>().unwrap().compositor_state
    }

    fn commit(&mut self, surface: &WlSurface) {
        on_commit_buffer_handler::<Self>(surface);
        self.popups.commit(surface);
        self.space.elements().for_each(|window| window.on_commit());

        // Check if this surface is a window surface, popup, or subsurface (not a tiny cursor)
        let is_valid_window = self.space.elements().any(|window| {
            window.toplevel().map(|t| t.wl_surface() == surface).unwrap_or(false)
        }) || self.popups.find_popup(surface).is_some();

        if is_valid_window {
            // Extract committed pixel buffer and queue for parent forwarding
            let frame = with_renderer_surface_state(surface, |state| {
                state.buffer().and_then(|buffer| {
                    with_buffer_contents(buffer, |ptr, len, data| {
                        if data.width >= 48 && data.height >= 48 {
                            let bytes = unsafe { std::slice::from_raw_parts(ptr, len) }.to_vec();
                            Some(CommittedFrame {
                                width: data.width,
                                height: data.height,
                                stride: data.stride,
                                bytes,
                            })
                        } else {
                            None
                        }
                    }).ok().flatten()
                })
            }).flatten();

            if let Some(f) = frame {
                debug!("🎨 [ISLAND FRAME CAPTURED] Surface committed buffer ({}x{})", f.width, f.height);
                self.pending_frames.push(f);
            }
        }
    }
}

impl BufferHandler for IslandState {
    fn buffer_destroyed(&mut self, _buffer: &WlBuffer) {}
}

impl ShmHandler for IslandState {
    fn shm_state(&self) -> &ShmState {
        &self.shm_state
    }
}

impl SeatHandler for IslandState {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<IslandState> {
        &mut self.seat_state
    }

    fn focus_changed(&mut self, _seat: &Seat<Self>, _focused: Option<&WlSurface>) {}
    fn cursor_image(&mut self, _seat: &Seat<Self>, _image: smithay::input::pointer::CursorImageStatus) {}
}

impl XdgShellHandler for IslandState {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        let window = Window::new_wayland_window(surface.clone());
        info!("🏝️ [ISLAND GUEST TOPLEVEL] Created isolated window: {:?}", surface.wl_surface().id());

        // Configure initial size
        surface.with_pending_state(|state| {
            state.size = Some((900, 600).into());
            state.states.set(smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State::Activated);
        });
        surface.send_configure();

        self.space.map_element(window, (0, 0), true);
    }

    fn new_popup(&mut self, surface: PopupSurface, _positioner: PositionerState) {
        info!("✨ [ISLAND POPUP] Tracking and configuring island popup: {:?}", surface.wl_surface().id());
        if let Err(err) = self.popups.track_popup(PopupKind::Xdg(surface.clone())) {
            warn!("Failed to track island popup: {:?}", err);
        }
        if let Err(err) = surface.send_configure() {
            warn!("Failed to send island popup configure: {:?}", err);
        }
    }

    fn move_request(&mut self, _surface: ToplevelSurface, _seat: smithay::reexports::wayland_server::protocol::wl_seat::WlSeat, _serial: Serial) {}
    fn grab(&mut self, _surface: PopupSurface, _seat: smithay::reexports::wayland_server::protocol::wl_seat::WlSeat, _serial: Serial) {}
    fn reposition_request(&mut self, _surface: PopupSurface, _positioner: PositionerState, _token: u32) {}
}

impl XdgDecorationHandler for IslandState {
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

impl SelectionHandler for IslandState {
    type SelectionUserData = ();
}

impl DataDeviceHandler for IslandState {
    fn data_device_state(&self) -> &DataDeviceState {
        &self.data_device_state
    }
}

impl ClientDndGrabHandler for IslandState {}
impl ServerDndGrabHandler for IslandState {}
impl OutputHandler for IslandState {}
