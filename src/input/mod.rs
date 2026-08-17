pub mod simulator;

use smithay::{
    backend::input::Keycode,
    desktop::WindowSurfaceType,
    input::{
        keyboard::FilterResult,
        pointer::{ButtonEvent, MotionEvent},
    },
    reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State as XdgState,
    utils::{Logical, Point, SERIAL_COUNTER},
};
use tracing::info;
use crate::state::KnotState;

pub struct KnotInputHandler;

impl KnotInputHandler {
    pub fn handle_pointer_motion(state: &mut KnotState, location: Point<f64, Logical>) {
        let user = state.seat_manager.current_user_mut();
        user.location = location;
        let pointer = user.pointer.clone();

        // Handle window drag/move if dragging
        if let Some((ref window, offset)) = state.drag_state.clone() {
            let new_loc = Point::from(((location.x - offset.x) as i32, (location.y - offset.y) as i32));
            state.space.map_element(window.clone(), new_loc, false);
        }

        // Find surface and its global origin in compositor space
        let mut focus = None;
        for window in state.space.elements() {
            if let Some(win_loc) = state.space.element_location(window) {
                let rel_point = location - win_loc.to_f64();
                if let Some((surface, surf_offset)) = window.surface_under(rel_point, WindowSurfaceType::ALL) {
                    // Global origin of the surface in compositor coordinates
                    let surface_global_origin = win_loc.to_f64() + surf_offset.to_f64();
                    focus = Some((surface, surface_global_origin));
                    break;
                }
            }
        }

        let serial = SERIAL_COUNTER.next_serial();
        pointer.motion(
            state,
            focus,
            &MotionEvent {
                location,
                serial,
                time: state.start_time.elapsed().as_millis() as u32,
            },
        );

        // Crucial for Wayland v5+ clients: send frame delimiter!
        pointer.frame(state);
    }

    pub fn handle_pointer_button(state: &mut KnotState, button: u32, pressed: bool) {
        let user = state.seat_manager.current_user_mut();
        let location = user.location;
        let pointer = user.pointer.clone();
        let keyboard = user.keyboard.clone();
        let user_name = user.name.clone();
        let user_color = user.color_hex;

        let serial = SERIAL_COUNTER.next_serial();

        info!(
            "🖱️ [MOUSE CLICK] User: {} | Button: {} | Pressed: {} | Pos: ({:.1}, {:.1})",
            user_name, button, pressed, location.x, location.y
        );

        pointer.button(
            state,
            &ButtonEvent {
                serial,
                time: state.start_time.elapsed().as_millis() as u32,
                button,
                state: if pressed {
                    smithay::backend::input::ButtonState::Pressed
                } else {
                    smithay::backend::input::ButtonState::Released
                },
            },
        );

        // Send pointer frame delimiter
        pointer.frame(state);

        if pressed {
            // Find window and surface under click
            let mut found_target = None;
            for window in state.space.elements() {
                if let Some(win_loc) = state.space.element_location(window) {
                    let rel_point = location - win_loc.to_f64();
                    if let Some((surface, _)) = window.surface_under(rel_point, WindowSurfaceType::ALL) {
                        found_target = Some((window.clone(), surface, win_loc));
                        break;
                    }
                }
            }

            if let Some((window, surface, loc)) = found_target {
                info!(
                    "🎯 [WINDOW FOCUS GRABBED] User: {} ({}) | Window at ({}, {}) | Setting Keyboard Focus!",
                    user_name, user_color, loc.x, loc.y
                );

                // Raise window to top
                state.space.raise_element(&window, true);

                // Set keyboard focus for THIS user seat
                keyboard.set_focus(state, Some(surface), serial);

                // Mark toplevel as Activated so GTK/Chromium enables input
                if let Some(toplevel) = window.toplevel() {
                    toplevel.with_pending_state(|s| {
                        s.states.set(XdgState::Activated);
                    });
                    toplevel.send_configure();
                }
            } else {
                info!("⚠️ [CLICK ON EMPTY CANVAS] No window at position ({:.1}, {:.1})", location.x, location.y);
            }
        } else {
            if state.drag_state.is_some() {
                info!("🛑 [DRAG RELEASED]");
            }
            state.drag_state = None;
        }

        // Flush immediately to ensure zero-latency delivery
        let _ = state.display_handle.flush_clients();
    }

    pub fn handle_key(state: &mut KnotState, keycode: Keycode, pressed: bool) {
        let user = state.seat_manager.current_user();
        let keyboard = user.keyboard.clone();
        let serial = SERIAL_COUNTER.next_serial();

        info!(
            "⌨️ [FORWARDING KEY] User: {} | Keycode: {:?} ({}) | Pressed: {}",
            user.name, keycode, keycode.raw(), pressed
        );

        keyboard.input::<(), _>(
            state,
            keycode,
            if pressed {
                smithay::backend::input::KeyState::Pressed
            } else {
                smithay::backend::input::KeyState::Released
            },
            serial,
            state.start_time.elapsed().as_millis() as u32,
            |_, _, _| FilterResult::Forward,
        );

        // Flush immediately
        let _ = state.display_handle.flush_clients();
    }
}
