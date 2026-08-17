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
        }
    }
}
