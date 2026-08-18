use smithay::{
    backend::input::Keycode,
    input::{
        keyboard::{FilterResult, KeyboardHandle},
        pointer::{ButtonEvent, MotionEvent, PointerHandle},
        Seat,
    },
    utils::{Logical, Point, SERIAL_COUNTER},
};
use tracing::{debug, info};
use super::server::IslandState;

pub struct SeatMultiplexer {
    pub virtual_seat: Option<Seat<IslandState>>,
    pub virtual_pointer: Option<PointerHandle<IslandState>>,
    pub virtual_keyboard: Option<KeyboardHandle<IslandState>>,
    pub active_seat_name: String,
}

impl SeatMultiplexer {
    pub fn new() -> Self {
        Self {
            virtual_seat: None,
            virtual_pointer: None,
            virtual_keyboard: None,
            active_seat_name: "seat-alice".to_string(),
        }
    }

    pub fn set_virtual_seat(&mut self, seat: Seat<IslandState>) {
        let pointer = seat.get_pointer();
        let keyboard = seat.get_keyboard();
        self.virtual_seat = Some(seat);
        self.virtual_pointer = pointer;
        self.virtual_keyboard = keyboard;
        info!("🌟 [ISLAND MULTIPLEXER] Virtual single-seat initialized for guest application");
    }
}

impl IslandState {
    pub fn handle_parent_pointer_motion(
        &mut self,
        source_seat: &str,
        location: Point<f64, Logical>,
    ) {
        if self.multiplexer.active_seat_name != source_seat {
            debug!("🔄 [ISLAND SEAT SWITCH] Pointer control transitioned from {} to {}", self.multiplexer.active_seat_name, source_seat);
            self.multiplexer.active_seat_name = source_seat.to_string();
        }

        if let Some(pointer) = self.multiplexer.virtual_pointer.clone() {
            let under = self.get_surface_under(location);
            let serial = SERIAL_COUNTER.next_serial();
            pointer.motion(
                self,
                under,
                &MotionEvent {
                    location,
                    serial,
                    time: self.start_time.elapsed().as_millis() as u32,
                },
            );
            pointer.frame(self);
            let _ = self.display_handle.flush_clients();
        }
    }

    pub fn handle_parent_pointer_button(
        &mut self,
        source_seat: &str,
        button: u32,
        pressed: bool,
    ) {
        if self.multiplexer.active_seat_name != source_seat {
            info!("🎯 [ISLAND FOCUS HANDOFF] Seat {} clicked window: Switching active multiplexer stream", source_seat);
            self.multiplexer.active_seat_name = source_seat.to_string();
        }

        let virtual_pointer = self.multiplexer.virtual_pointer.clone();
        let virtual_keyboard = self.multiplexer.virtual_keyboard.clone();

        if let Some(pointer) = virtual_pointer {
            let serial = SERIAL_COUNTER.next_serial();
            pointer.button(
                self,
                &ButtonEvent {
                    serial,
                    time: self.start_time.elapsed().as_millis() as u32,
                    button,
                    state: if pressed {
                        smithay::backend::input::ButtonState::Pressed
                    } else {
                        smithay::backend::input::ButtonState::Released
                    },
                },
            );
            pointer.frame(self);

            if pressed {
                // Focus the child app's root toplevel surface and activate window
                let root_surface = self.get_root_surface();
                if let Some(keyboard) = virtual_keyboard {
                    keyboard.set_focus(self, root_surface, serial);
                }
                for window in self.space.elements() {
                    if let Some(toplevel) = window.toplevel() {
                        toplevel.with_pending_state(|s| {
                            s.states.set(smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State::Activated);
                        });
                        toplevel.send_configure();
                    }
                }
            }

            let _ = self.display_handle.flush_clients();
        }
    }

    pub fn handle_parent_keyboard_key(
        &mut self,
        source_seat: &str,
        keycode: Keycode,
        pressed: bool,
    ) {
        info!(
            "⌨️ [ISLAND MULTIPLEXED KEY] From Seat: {} -> Guest default-seat | Keycode: {:?} | Pressed: {}",
            source_seat, keycode, pressed
        );

        if let Some(keyboard) = self.multiplexer.virtual_keyboard.clone() {
            let serial = SERIAL_COUNTER.next_serial();
            keyboard.input::<(), _>(
                self,
                keycode,
                if pressed {
                    smithay::backend::input::KeyState::Pressed
                } else {
                    smithay::backend::input::KeyState::Released
                },
                serial,
                self.start_time.elapsed().as_millis() as u32,
                |_, _, _| FilterResult::Forward,
            );
            let _ = self.display_handle.flush_clients();
        }
    }
}
