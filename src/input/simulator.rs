use smithay::{
    backend::input::Keycode,
    input::keyboard::FilterResult,
    utils::SERIAL_COUNTER,
};
use tracing::info;
use crate::state::KnotState;

pub struct ConcurrentFocusSimulator {
    pub step: u64,
    pub active: bool,
}

impl ConcurrentFocusSimulator {
    pub fn new() -> Self {
        Self { step: 0, active: false }
    }

    pub fn tick(&mut self, state: &mut KnotState) {
        if !self.active {
            return;
        }

        let windows: Vec<_> = state.space.elements().cloned().collect();
        if windows.len() < 2 {
            if self.step % 60 == 0 {
                info!("⚠️ Simulator waiting for 2 windows to open... (Current: {})", windows.len());
            }
            self.step += 1;
            return;
        }

        let win1_surface = windows[0].toplevel().map(|t| t.wl_surface().clone());
        let win2_surface = windows[1].toplevel().map(|t| t.wl_surface().clone());

        if let (Some(surf1), Some(surf2)) = (win1_surface, win2_surface) {
            let alice_kb = state.seat_manager.alice().keyboard.clone();
            let bob_kb = state.seat_manager.bob().keyboard.clone();

            let serial1 = SERIAL_COUNTER.next_serial();
            let serial2 = SERIAL_COUNTER.next_serial();

            // Ensure Alice is focused on Window 1, and Bob is focused on Window 2
            alice_kb.set_focus(state, Some(surf1), serial1);
            bob_kb.set_focus(state, Some(surf2), serial2);

            // Periodically inject concurrent keystrokes
            if self.step % 30 == 0 {
                // Linux keycode 38 = 'a' (Alice), 56 = 'b' (Bob)
                let time = state.start_time.elapsed().as_millis() as u32;

                // Press and release 'A' in Alice's seat (Window 1)
                alice_kb.input::<(), _>(state, Keycode::new(38), smithay::backend::input::KeyState::Pressed, serial1, time, |_, _, _| FilterResult::Forward);
                alice_kb.input::<(), _>(state, Keycode::new(38), smithay::backend::input::KeyState::Released, serial1, time + 5, |_, _, _| FilterResult::Forward);

                // Press and release 'B' in Bob's seat (Window 2)
                bob_kb.input::<(), _>(state, Keycode::new(56), smithay::backend::input::KeyState::Pressed, serial2, time, |_, _, _| FilterResult::Forward);
                bob_kb.input::<(), _>(state, Keycode::new(56), smithay::backend::input::KeyState::Released, serial2, time + 5, |_, _, _| FilterResult::Forward);

                info!("✨ [CONCURRENT INPUT] Injected 'A' -> Win 1 (Alice 💖) & 'B' -> Win 2 (Bob 💙)");
            }
        }

        self.step += 1;
    }
}
