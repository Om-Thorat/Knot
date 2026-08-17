pub mod user;

use smithay::{
    input::{
        keyboard::XkbConfig,
        SeatState,
    },
    reexports::wayland_server::DisplayHandle,
    utils::Point,
};
use tracing::info;
use user::KnotUser;
use crate::state::KnotState;

pub struct KnotSeatManager {
    pub users: Vec<KnotUser>,
    pub active_user_index: usize, // For simulator switching (0 = Alice, 1 = Bob)
}

impl KnotSeatManager {
    pub fn new(dh: &DisplayHandle, seat_state: &mut SeatState<KnotState>) -> Self {
        // Create Seat 1: Alice (Host - Pink)
        let mut seat_alice = seat_state.new_wl_seat(dh, "seat-alice");
        let keyboard_alice = seat_alice
            .add_keyboard(XkbConfig::default(), 200, 25)
            .expect("Failed to initialize Alice's keyboard");
        let pointer_alice = seat_alice.add_pointer();

        let alice = KnotUser::new(
            1,
            "Alice (Host)".to_string(),
            [1.0, 0.25, 0.51], // #FF4081 Pink
            "#FF4081",
            Point::from((200.0, 200.0)),
            seat_alice,
            pointer_alice,
            keyboard_alice,
        );

        // Create Seat 2: Bob (Guest - Cyan)
        let mut seat_bob = seat_state.new_wl_seat(dh, "seat-bob");
        let keyboard_bob = seat_bob
            .add_keyboard(XkbConfig::default(), 200, 25)
            .expect("Failed to initialize Bob's keyboard");
        let pointer_bob = seat_bob.add_pointer();

        let bob = KnotUser::new(
            2,
            "Bob (Guest)".to_string(),
            [0.0, 0.90, 1.0], // #00E5FF Cyan
            "#00E5FF",
            Point::from((600.0, 200.0)),
            seat_bob,
            pointer_bob,
            keyboard_bob,
        );

        info!("Initialized Dual-Seat Engine: Alice (Pink) and Bob (Cyan)");

        Self {
            users: vec![alice, bob],
            active_user_index: 0,
        }
    }

    pub fn alice(&self) -> &KnotUser {
        &self.users[0]
    }

    pub fn bob(&self) -> &KnotUser {
        &self.users[1]
    }

    pub fn current_user(&self) -> &KnotUser {
        &self.users[self.active_user_index]
    }

    pub fn current_user_mut(&mut self) -> &mut KnotUser {
        &mut self.users[self.active_user_index]
    }

    pub fn toggle_active_user(&mut self) -> &KnotUser {
        self.active_user_index = (self.active_user_index + 1) % self.users.len();
        &self.users[self.active_user_index]
    }
}
