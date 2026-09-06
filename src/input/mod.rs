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
    fn raw_to_char(raw: u32) -> Option<char> {
        match raw {
            65 => Some(' '),
            24 => Some('q'),
            25 => Some('w'),
            26 => Some('e'),
            27 => Some('r'),
            28 => Some('t'),
            29 => Some('y'),
            30 => Some('u'),
            31 => Some('i'),
            32 => Some('o'),
            33 => Some('p'),
            38 => Some('a'),
            39 => Some('s'),
            40 => Some('d'),
            41 => Some('f'),
            42 => Some('g'),
            43 => Some('h'),
            44 => Some('j'),
            45 => Some('k'),
            46 => Some('l'),
            52 => Some('z'),
            53 => Some('x'),
            54 => Some('c'),
            55 => Some('v'),
            56 => Some('b'),
            57 => Some('n'),
            58 => Some('m'),
            10 => Some('1'),
            11 => Some('2'),
            12 => Some('3'),
            13 => Some('4'),
            14 => Some('5'),
            15 => Some('6'),
            16 => Some('7'),
            17 => Some('8'),
            18 => Some('9'),
            19 => Some('0'),
            20 => Some('-'),
            60 => Some('.'),
            59 => Some(','),
            _ => None,
        }
    }

    pub fn handle_pointer_motion(state: &mut KnotState, location: Point<f64, Logical>) {
        let user = state.seat_manager.current_user_mut();
        user.location = location;
        let pointer = user.pointer.clone();

        // Handle active window drag/move if dragging
        if let Some((ref window, offset)) = state.drag_state.clone() {
            let new_loc = Point::from(((location.x - offset.x) as i32, (location.y - offset.y) as i32));
            state.space.map_element(window.clone(), new_loc, false);
        }

        // If launcher is open, update selected row on hover
        if state.launcher_state.is_open {
            let modal_x = 320.0;
            let modal_w = 640.0;
            let row_start_y = 192.0;

            if location.x >= modal_x + 14.0 && location.x <= modal_x + modal_w - 14.0 && location.y >= row_start_y {
                let row_idx = ((location.y - row_start_y) / 58.0) as usize;
                let filtered = state.launcher_state.filtered_catalog();
                if row_idx < filtered.len() {
                    state.launcher_state.selected_index = row_idx;
                }
            }
        }

        // Find surface and its global origin in compositor space
        let mut focus = None;
        if !state.launcher_state.is_open {
            for window in state.space.elements() {
                if let Some(win_loc) = state.space.element_location(window) {
                    let rel_point = location - win_loc.to_f64();
                    if let Some((surface, surf_offset)) = window.surface_under(rel_point, WindowSurfaceType::ALL) {
                        let surface_global_origin = win_loc.to_f64() + surf_offset.to_f64();
                        focus = Some((surface, surface_global_origin));
                        break;
                    }
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

        // 1. Check if clicking on Top HUD [+ LAUNCH APP] Button
        if pressed && button == 272 && location.x >= 650.0 && location.x <= 890.0 && location.y >= 10.0 && location.y <= 42.0 {
            info!("🎯 [LAUNCHER BUTTON CLICKED] Toggling App Palette!");
            state.toggle_launcher();
            return;
        }

        // 2. Handle clicks when App Launcher is Open
        if state.launcher_state.is_open {
            if pressed && button == 272 {
                let modal_x = 320.0;
                let modal_w = 640.0;
                let modal_y = 100.0;
                let modal_h = 510.0;
                let row_start_y = 192.0;

                // Click inside catalog rows
                if location.x >= modal_x + 14.0 && location.x <= modal_x + modal_w - 14.0 && location.y >= row_start_y && location.y <= modal_y + modal_h - 10.0 {
                    let row_idx = ((location.y - row_start_y) / 58.0) as usize;
                    let filtered = state.launcher_state.filtered_catalog();
                    if row_idx < filtered.len() {
                        info!("🚀 User clicked app row #{} in catalog: '{}'!", row_idx + 1, filtered[row_idx].title);
                        state.launch_filtered_index(row_idx);
                        return;
                    }
                }

                // Click outside modal closes it
                if location.x < modal_x || location.x > modal_x + modal_w || location.y < modal_y || location.y > modal_y + modal_h {
                    state.launcher_state.is_open = false;
                    info!("🚪 Clicked outside modal, closing launcher.");
                    return;
                }
            }
            return;
        }

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

                // Check if clicking in the top titlebar / header region (y <= 42px) to initiate drag
                let rel_y = location.y - loc.y as f64;
                let rel_x = location.x - loc.x as f64;
                let win_geo = window.geometry();

                if button == 272 && rel_y <= 42.0 && rel_x < (win_geo.size.w as f64 - 100.0) {
                    info!("✋ [HEADER DRAG INITIATED] Dragging window from titlebar");
                    state.drag_state = Some((window.clone(), Point::from((location.x - loc.x as f64, location.y - loc.y as f64))));
                }

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

        let _ = state.display_handle.flush_clients();
    }

    pub fn handle_key(state: &mut KnotState, keycode: Keycode, pressed: bool) {
        let user = state.seat_manager.current_user();
        let keyboard = user.keyboard.clone();
        let serial = SERIAL_COUNTER.next_serial();
        let raw = keycode.raw();

        info!(
            "⌨️ [KEY EVENT] User: {} | Keycode: {:?} (raw: {}) | Pressed: {}",
            user.name, keycode, raw, pressed
        );

        // Check for Super/Windows key (raw 133 or 125) to toggle launcher
        if pressed && (raw == 133 || raw == 125) {
            info!("🌟 [SUPER KEY] Toggling App Launcher!");
            state.toggle_launcher();
            return;
        }

        // Handle navigation and search typing when Launcher is open
        if state.launcher_state.is_open {
            if pressed {
                match raw {
                    // Escape (XKB 9 / evdev 1)
                    9 | 1 => {
                        state.launcher_state.is_open = false;
                        info!("🚪 [ESC] Closing launcher modal.");
                        return;
                    }
                    // Backspace (XKB 22 / evdev 14)
                    22 | 14 => {
                        state.launcher_state.query.pop();
                        state.launcher_state.selected_index = 0;
                        info!("🔍 [SEARCH BACKSPACE] Query: '{}'", state.launcher_state.query);
                        return;
                    }
                    // Up Arrow (XKB 111 / evdev 103)
                    111 | 103 => {
                        state.launcher_select_prev();
                        return;
                    }
                    // Down Arrow (XKB 116 / evdev 108)
                    116 | 108 => {
                        state.launcher_select_next();
                        return;
                    }
                    // Enter / Return (XKB 36 / evdev 28)
                    36 | 28 => {
                        state.launch_selected();
                        return;
                    }
                    _ => {
                        // Quick 1-key launch with numbers if query is empty
                        if state.launcher_state.query.is_empty() && (10..=16).contains(&raw) {
                            let idx = (raw - 10) as usize;
                            if idx < state.launcher_state.catalog.len() {
                                state.launch_filtered_index(idx);
                                return;
                            }
                        }

                        // Otherwise append character to search query
                        if let Some(ch) = Self::raw_to_char(raw) {
                            state.launcher_state.query.push(ch);
                            state.launcher_state.selected_index = 0;
                            info!("🔍 [SEARCH TYPED] Query: '{}'", state.launcher_state.query);
                            return;
                        }
                    }
                }
            }
            return;
        }

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

        let _ = state.display_handle.flush_clients();
    }

    pub fn handle_remote_pointer_motion(state: &mut KnotState, location: Point<f64, Logical>) {
        state.seat_manager.bob_mut().location = location;
        let pointer = state.seat_manager.bob().pointer.clone();

        // Handle active window dragging
        if let Some((ref window, grab_offset)) = state.drag_state {
            let new_loc = Point::from(((location.x - grab_offset.x) as i32, (location.y - grab_offset.y) as i32));
            state.space.map_element(window.clone(), new_loc, false);
            return;
        }

        let mut focus = None;
        if !state.launcher_state.is_open {
            for window in state.space.elements() {
                if let Some(win_loc) = state.space.element_location(window) {
                    let rel_point = location - win_loc.to_f64();
                    if let Some((surface, surf_offset)) = window.surface_under(rel_point, WindowSurfaceType::ALL) {
                        let surface_global_origin = win_loc.to_f64() + surf_offset.to_f64();
                        focus = Some((surface, surface_global_origin));
                        break;
                    }
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
        pointer.frame(state);
        let _ = state.display_handle.flush_clients();
    }

    pub fn handle_remote_pointer_button(state: &mut KnotState, button: u32, pressed: bool) {
        let location = state.seat_manager.bob().location;
        let pointer = state.seat_manager.bob().pointer.clone();
        let keyboard = state.seat_manager.bob().keyboard.clone();
        let user_name = state.seat_manager.bob().name.clone();
        let user_color = state.seat_manager.bob().color_hex;

        let serial = SERIAL_COUNTER.next_serial();

        info!(
            "🌐🖱️ [REMOTE MOUSE CLICK] User: {} | Button: {} | Pressed: {} | Pos: ({:.1}, {:.1})",
            user_name, button, pressed, location.x, location.y
        );

        // 1. Check if clicking on Top HUD [+ LAUNCH APP] Button
        if pressed && button == 272 && location.x >= 550.0 && location.x <= 790.0 && location.y >= 10.0 && location.y <= 42.0 {
            info!("🎯 [REMOTE LAUNCHER BUTTON CLICKED] Toggling App Palette!");
            state.toggle_launcher();
            return;
        }

        // 2. Handle clicks when App Launcher is Open
        if state.launcher_state.is_open {
            if pressed && button == 272 {
                let modal_x = 320.0;
                let modal_w = 640.0;
                let modal_y = 100.0;
                let modal_h = 510.0;
                let row_start_y = 192.0;

                // Click inside catalog rows
                if location.x >= modal_x + 14.0 && location.x <= modal_x + modal_w - 14.0 && location.y >= row_start_y && location.y <= modal_y + modal_h - 10.0 {
                    let row_idx = ((location.y - row_start_y) / 58.0) as usize;
                    let filtered = state.launcher_state.filtered_catalog();
                    if row_idx < filtered.len() {
                        info!("🚀 Remote user clicked app row #{} in catalog: '{}'!", row_idx + 1, filtered[row_idx].title);
                        state.launch_filtered_index(row_idx);
                        return;
                    }
                }

                // Click outside modal closes it
                if location.x < modal_x || location.x > modal_x + modal_w || location.y < modal_y || location.y > modal_y + modal_h {
                    state.launcher_state.is_open = false;
                    info!("🚪 Remote clicked outside modal, closing launcher.");
                    return;
                }
            }
            return;
        }

        // Resolve surface and global origin for the click position
        let mut found_target = None;
        let mut focus = None;
        if !state.launcher_state.is_open {
            for window in state.space.elements() {
                if let Some(win_loc) = state.space.element_location(window) {
                    let rel_point = location - win_loc.to_f64();
                    if let Some((surface, surf_offset)) = window.surface_under(rel_point, WindowSurfaceType::ALL) {
                        let surface_global_origin = win_loc.to_f64() + surf_offset.to_f64();
                        focus = Some((surface.clone(), surface_global_origin));
                        found_target = Some((window.clone(), surface, win_loc));
                        break;
                    }
                }
            }
        }

        // 1. Ensure pointer has focus on the target surface so the button event is dispatched to the client
        pointer.motion(
            state,
            focus,
            &MotionEvent {
                location,
                serial,
                time: state.start_time.elapsed().as_millis() as u32,
            },
        );

        // 2. Dispatch the button press/release event
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
        pointer.frame(state);

        if pressed {
            if let Some((window, surface, loc)) = found_target {
                info!(
                    "🎯 [REMOTE WINDOW FOCUS GRABBED] User: {} ({}) | Window at ({}, {}) | Setting Keyboard Focus!",
                    user_name, user_color, loc.x, loc.y
                );
                state.space.raise_element(&window, true);

                // Check if clicking in the top titlebar / header region (y <= 42px) to initiate drag
                let rel_y = location.y - loc.y as f64;
                let rel_x = location.x - loc.x as f64;
                let win_geo = window.geometry();

                if button == 272 && rel_y <= 42.0 && rel_x < (win_geo.size.w as f64 - 100.0) {
                    info!("✋ [REMOTE HEADER DRAG INITIATED] Dragging window from titlebar");
                    state.drag_state = Some((window.clone(), Point::from((location.x - loc.x as f64, location.y - loc.y as f64))));
                }

                keyboard.set_focus(state, Some(surface), serial);
                if let Some(toplevel) = window.toplevel() {
                    toplevel.with_pending_state(|s| {
                        s.states.set(XdgState::Activated);
                    });
                    toplevel.send_configure();
                }
            }
        } else {
            if state.drag_state.is_some() {
                info!("🛑 [REMOTE DRAG RELEASED]");
            }
            state.drag_state = None;
        }
        let _ = state.display_handle.flush_clients();
    }

    pub fn handle_remote_key(state: &mut KnotState, keycode: Keycode, pressed: bool) {
        let keyboard = state.seat_manager.bob().keyboard.clone();
        let serial = SERIAL_COUNTER.next_serial();
        let raw = keycode.raw();

        info!(
            "🌐⌨️ [REMOTE KEY EVENT] User: {} | Keycode: {:?} (raw: {}) | Pressed: {}",
            state.seat_manager.bob().name, keycode, raw, pressed
        );

        // Check for Super/Windows key (raw 133 or 125) to toggle launcher
        if pressed && (raw == 133 || raw == 125) {
            info!("🌟 [REMOTE SUPER KEY] Toggling App Launcher!");
            state.toggle_launcher();
            return;
        }

        // Handle navigation and search typing when Launcher is open
        if state.launcher_state.is_open {
            if pressed {
                match raw {
                    // Escape (XKB 9 / evdev 1)
                    9 | 1 => {
                        state.launcher_state.is_open = false;
                        info!("🚪 [ESC] Remote closed launcher modal.");
                        return;
                    }
                    // Backspace (XKB 22 / evdev 14)
                    22 | 14 => {
                        state.launcher_state.query.pop();
                        state.launcher_state.selected_index = 0;
                        info!("🔍 [REMOTE SEARCH BACKSPACE] Query: '{}'", state.launcher_state.query);
                        return;
                    }
                    // Up Arrow (XKB 111 / evdev 103)
                    111 | 103 => {
                        state.launcher_select_prev();
                        return;
                    }
                    // Down Arrow (XKB 116 / evdev 108)
                    116 | 108 => {
                        state.launcher_select_next();
                        return;
                    }
                    // Enter / Return (XKB 36 / evdev 28)
                    36 | 28 => {
                        state.launch_selected();
                        return;
                    }
                    _ => {
                        // Quick 1-key launch with numbers if query is empty
                        if state.launcher_state.query.is_empty() && (10..=16).contains(&raw) {
                            let idx = (raw - 10) as usize;
                            if idx < state.launcher_state.catalog.len() {
                                state.launch_filtered_index(idx);
                                return;
                            }
                        }

                        // Otherwise append character to search query
                        if let Some(ch) = Self::raw_to_char(raw) {
                            state.launcher_state.query.push(ch);
                            state.launcher_state.selected_index = 0;
                            info!("🔍 [REMOTE SEARCH TYPED] Query: '{}'", state.launcher_state.query);
                            return;
                        }
                    }
                }
            }
            return;
        }

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
        let _ = state.display_handle.flush_clients();
    }
}
