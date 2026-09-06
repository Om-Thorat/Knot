use std::time::Instant;
use smithay::{
    input::{
        keyboard::KeyboardHandle,
        pointer::PointerHandle,
        Seat,
    },
    utils::{Logical, Point},
};
use crate::state::KnotState;

#[derive(Debug, Clone)]
pub struct KnotUser {
    pub id: u32,
    pub name: String,
    pub color_rgb: [f32; 3], // RGB 0.0 - 1.0
    pub color_hex: &'static str,
    pub location: Point<f64, Logical>,
    pub seat: Seat<KnotState>,
    pub pointer: PointerHandle<KnotState>,
    pub keyboard: KeyboardHandle<KnotState>,
    pub focused_window_title: Option<String>,
    pub cursor_chat: Option<(String, Instant)>,
    pub is_remote: bool,
}

impl KnotUser {
    pub fn new(
        id: u32,
        name: String,
        color_rgb: [f32; 3],
        color_hex: &'static str,
        initial_location: Point<f64, Logical>,
        seat: Seat<KnotState>,
        pointer: PointerHandle<KnotState>,
        keyboard: KeyboardHandle<KnotState>,
    ) -> Self {
        Self {
            id,
            name,
            color_rgb,
            color_hex,
            location: initial_location,
            seat,
            pointer,
            keyboard,
            focused_window_title: None,
            cursor_chat: None,
            is_remote: false,
        }
    }

    pub fn set_cursor_chat(&mut self, text: String) {
        self.cursor_chat = Some((text, Instant::now()));
    }

    pub fn active_cursor_chat(&self) -> Option<&str> {
        if let Some((ref text, timestamp)) = self.cursor_chat {
            // Keep speech bubble visible for 5 seconds
            if timestamp.elapsed().as_secs() < 5 {
                return Some(text.as_str());
            }
        }
        None
    }
}
