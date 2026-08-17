pub mod cursors;
pub mod text;

use smithay::{
    backend::renderer::{
        damage::OutputDamageTracker,
        element::{
            memory::MemoryRenderBufferRenderElement,
            render_elements,
            solid::{SolidColorBuffer, SolidColorRenderElement},
            surface::WaylandSurfaceRenderElement,
            Kind,
        },
        gles::GlesTarget,
        glow::GlowRenderer,
        Color32F,
    },
    desktop::{
        space::{space_render_elements, SpaceRenderElements},
        Window,
    },
    utils::{Point, Size},
};
use crate::state::KnotState;
use text::BadgeTextRenderer;

render_elements! {
    pub KnotRenderElement<=GlowRenderer>;
    Space=SpaceRenderElements<GlowRenderer, WaylandSurfaceRenderElement<GlowRenderer>>,
    Solid=SolidColorRenderElement,
    Memory=MemoryRenderBufferRenderElement<GlowRenderer>,
}

pub struct KnotRenderer {
    badge_renderer: BadgeTextRenderer,
}

impl KnotRenderer {
    pub fn new() -> Self {
        Self {
            badge_renderer: BadgeTextRenderer::new(),
        }
    }

    #[inline]
    fn create_rect(
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        color: Color32F,
    ) -> KnotRenderElement {
        let buf = SolidColorBuffer::new(Size::from((w, h)), color);
        KnotRenderElement::Solid(SolidColorRenderElement::from_buffer(
            &buf,
            Point::from((x, y)),
            1.0,
            1.0,
            Kind::Unspecified,
        ))
    }

    pub fn render_frame(
        &mut self,
        state: &mut KnotState,
        renderer: &mut GlowRenderer,
        framebuffer: &mut GlesTarget<'_>,
        damage_tracker: &mut OutputDamageTracker,
        age: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let output = &state.output;

        // Collect all window render elements from Space
        let space_elements = space_render_elements::<GlowRenderer, Window, _>(
            renderer,
            [&state.space],
            output,
            1.0,
        )?;

        let mut all_elements: Vec<KnotRenderElement> = Vec::new();

        // 1. Add background windows
        for elem in space_elements {
            all_elements.push(KnotRenderElement::Space(elem));
        }

        // 2. Render Figma-style Glowing multiplayer focus borders for each window
        let alice_color = Color32F::new(1.0, 0.25, 0.50, 0.95); // Pink #FF4081
        let bob_color = Color32F::new(0.0, 0.90, 1.0, 0.95);   // Cyan #00E5FF

        let alice_focused_surf = state.seat_manager.alice().keyboard.current_focus();
        let bob_focused_surf = state.seat_manager.bob().keyboard.current_focus();

        for window in state.space.elements() {
            if let Some(loc) = state.space.element_location(window) {
                let win_geo = window.geometry();
                let x = loc.x;
                let y = loc.y;
                let w = win_geo.size.w;
                let h = win_geo.size.h;

                let win_surf = window.toplevel().map(|t| t.wl_surface().clone());

                let is_alice_focused = win_surf.as_ref() == alice_focused_surf.as_ref();
                let is_bob_focused = win_surf.as_ref() == bob_focused_surf.as_ref();

                let border_info = if is_alice_focused && is_bob_focused {
                    Some((alice_color, "ALICE & BOB", [255, 64, 129, 240]))
                } else if is_alice_focused {
                    Some((alice_color, "ALICE", [255, 64, 129, 240]))
                } else if is_bob_focused {
                    Some((bob_color, "BOB", [0, 229, 255, 240]))
                } else {
                    None
                };

                if let Some((color, label, bg_rgba)) = border_info {
                    let border_thick = 4;
                    // Top glow bar
                    all_elements.push(Self::create_rect(
                        x - border_thick,
                        y - border_thick,
                        w + border_thick * 2,
                        border_thick,
                        color,
                    ));

                    // Bottom glow bar
                    all_elements.push(Self::create_rect(
                        x - border_thick,
                        y + h,
                        w + border_thick * 2,
                        border_thick,
                        color,
                    ));

                    // Left glow bar
                    all_elements.push(Self::create_rect(
                        x - border_thick,
                        y,
                        border_thick,
                        h,
                        color,
                    ));

                    // Right glow bar
                    all_elements.push(Self::create_rect(
                        x + w,
                        y,
                        border_thick,
                        h,
                        color,
                    ));

                    // Render User Name Pill with readable white text!
                    let badge_buf = self.badge_renderer.get_or_create_badge(
                        label,
                        bg_rgba,
                        [255, 255, 255, 255],
                    );
                    let badge_w = (label.chars().count() * 8 + 20) as i32;
                    if let Ok(elem) = MemoryRenderBufferRenderElement::from_buffer(
                        renderer,
                        Point::from(((x + w - badge_w) as f64, (y - 24) as f64)),
                        &badge_buf,
                        None,
                        None,
                        None,
                        Kind::Unspecified,
                    ) {
                        all_elements.push(KnotRenderElement::Memory(elem));
                    }
                }
            }
        }

        // 3. Render Multiplayer Cursors (Alice & Bob)
        let active_user_id = state.seat_manager.active_user_index;

        // Alice Cursor (Pink) + "ALICE" Pill Tag
        let alice_pos = state.seat_manager.alice().location;
        all_elements.push(Self::create_rect(
            alice_pos.x as i32,
            alice_pos.y as i32,
            12,
            12,
            alice_color,
        ));
        let alice_tag_buf = self.badge_renderer.get_or_create_badge(
            "ALICE",
            [255, 64, 129, 240],
            [255, 255, 255, 255],
        );
        if let Ok(elem) = MemoryRenderBufferRenderElement::from_buffer(
            renderer,
            Point::from(((alice_pos.x as i32 + 16) as f64, (alice_pos.y as i32 - 4) as f64)),
            &alice_tag_buf,
            None,
            None,
            None,
            Kind::Unspecified,
        ) {
            all_elements.push(KnotRenderElement::Memory(elem));
        }

        // Bob Cursor (Cyan) + "BOB" Pill Tag
        let bob_pos = state.seat_manager.bob().location;
        all_elements.push(Self::create_rect(
            bob_pos.x as i32,
            bob_pos.y as i32,
            12,
            12,
            bob_color,
        ));
        let bob_tag_buf = self.badge_renderer.get_or_create_badge(
            "BOB",
            [0, 229, 255, 240],
            [255, 255, 255, 255],
        );
        if let Ok(elem) = MemoryRenderBufferRenderElement::from_buffer(
            renderer,
            Point::from(((bob_pos.x as i32 + 16) as f64, (bob_pos.y as i32 - 4) as f64)),
            &bob_tag_buf,
            None,
            None,
            None,
            Kind::Unspecified,
        ) {
            all_elements.push(KnotRenderElement::Memory(elem));
        }

        // 4. Render Top Center Active Seat Indicator Pill with Text
        let (hud_text, hud_bg) = if active_user_id == 0 {
            ("ACTIVE: ALICE [TAB]", [255, 64, 129, 240])
        } else {
            ("ACTIVE: BOB [TAB]", [0, 229, 255, 240])
        };
        let hud_badge = self.badge_renderer.get_or_create_badge(
            hud_text,
            hud_bg,
            [255, 255, 255, 255],
        );
        if let Ok(elem) = MemoryRenderBufferRenderElement::from_buffer(
            renderer,
            Point::from((520.0, 14.0)),
            &hud_badge,
            None,
            None,
            None,
            Kind::Unspecified,
        ) {
            all_elements.push(KnotRenderElement::Memory(elem));
        }

        // Background color: Sleek dark canvas #0d1117 (R: 0.05, G: 0.07, B: 0.09)
        let clear_color = Color32F::new(0.051, 0.067, 0.090, 1.0);

        damage_tracker.render_output(
            renderer,
            framebuffer,
            age,
            &all_elements,
            clear_color,
        )?;

        Ok(())
    }
}
