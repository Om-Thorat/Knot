use smithay::{
    desktop::{Space, Window},
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point, Serial},
    wayland::{
        compositor::with_states,
        shell::xdg::{
            PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
            XdgToplevelSurfaceData,
        },
    },
};
use tracing::{info, warn};
use crate::state::KnotState;

impl XdgShellHandler for KnotState {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        let window = Window::new_wayland_toplevel(surface.clone());
        info!("New XDG toplevel window created: {:?}", surface.title());

        // Tile windows side-by-side or cascade them
        let count = self.space.elements().count();
        let x_pos = if count == 0 { 50 } else { (count * 350) % 900 + 50 };
        let y_pos = if count == 0 { 80 } else { ((count * 100) % 400) + 80 };

        self.space.map_element(window, (x_pos, y_pos), true);
        surface.send_configure();
    }

    fn new_popup(&mut self, surface: PopupSurface, _positioner: PositionerState) {
        // Popups (menus, dropdowns)
        if let Err(err) = self.popups.track_popup(smithay::desktop::PopupKind::Xdg(surface)) {
            warn!("Failed to track popup: {:?}", err);
        }
    }

    fn grab(&mut self, _surface: PopupSurface, _seat: smithay::reexports::wayland_server::protocol::wl_seat::WlSeat, _serial: Serial) {
        // Handle popup grabs
    }

    fn reposition_request(&mut self, _surface: PopupSurface, _positioner: PositionerState, _token: u32) {
        // Reposition popup if requested
    }
}
