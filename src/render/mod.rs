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

        // OutputDamageTracker expects elements in strict FRONT-TO-BACK (Top-to-Bottom) order!
        let mut all_elements: Vec<KnotRenderElement> = Vec::new();

        let alice_color = Color32F::new(1.0, 0.25, 0.50, 0.98); // Pink #FF4081
        let bob_color = Color32F::new(0.0, 0.90, 1.0, 0.98);   // Cyan #00E5FF
        let shadow_color = Color32F::new(0.0, 0.0, 0.0, 0.85);
        let white_color = Color32F::new(1.0, 1.0, 1.0, 1.0);
        let active_user_id = state.seat_manager.active_user_index;
        let active_theme_color = if active_user_id == 0 { alice_color } else { bob_color };
        let active_theme_rgba = if active_user_id == 0 { [255, 64, 129, 255] } else { [0, 229, 255, 255] };

        // =========================================================================
        // LAYER 1 (FRONT-MOST): Multiplayer Cursors & Floating User Tags
        // =========================================================================
        let draw_cursor = |elements: &mut Vec<KnotRenderElement>, pos: Point<f64, smithay::utils::Logical>, color: Color32F| {
            let px = pos.x as i32;
            let py = pos.y as i32;

            // Frontmost: Crosshair center point
            elements.push(Self::create_rect(px + 4, py + 4, 6, 6, color));
            elements.push(Self::create_rect(px + 3, py + 3, 8, 8, white_color));

            // Middle: User color body
            elements.push(Self::create_rect(px, py, 14, 14, color));

            // Back: White border and shadow
            elements.push(Self::create_rect(px - 1, py - 1, 16, 16, white_color));
            elements.push(Self::create_rect(px + 2, py + 2, 16, 16, shadow_color));
        };

        // Render Alice Cursor (Pink) + "ALICE" Badge
        let alice_pos = state.seat_manager.alice().location;
        let alice_tag_buf = self.badge_renderer.get_or_create_badge(
            "ALICE",
            [255, 64, 129, 255],
            [255, 255, 255, 255],
        );
        if let Ok(elem) = MemoryRenderBufferRenderElement::from_buffer(
            renderer,
            Point::from(((alice_pos.x as i32 + 18) as f64, (alice_pos.y as i32 - 4) as f64)),
            &alice_tag_buf,
            None,
            None,
            None,
            Kind::Unspecified,
        ) {
            all_elements.push(KnotRenderElement::Memory(elem));
        }
        draw_cursor(&mut all_elements, alice_pos, alice_color);

        // Render Bob Cursor (Cyan) + "BOB" Badge
        let bob_pos = state.seat_manager.bob().location;
        let bob_tag_buf = self.badge_renderer.get_or_create_badge(
            "BOB",
            [0, 229, 255, 255],
            [255, 255, 255, 255],
        );
        if let Ok(elem) = MemoryRenderBufferRenderElement::from_buffer(
            renderer,
            Point::from(((bob_pos.x as i32 + 18) as f64, (bob_pos.y as i32 - 4) as f64)),
            &bob_tag_buf,
            None,
            None,
            None,
            Kind::Unspecified,
        ) {
            all_elements.push(KnotRenderElement::Memory(elem));
        }
        draw_cursor(&mut all_elements, bob_pos, bob_color);

        // =========================================================================
        // LAYER 2: In-Compositor App Launcher Modal (if active)
        // =========================================================================
        if state.launcher_state.is_open {
            let modal_x = 320;
            let modal_y = 100;
            let modal_w = 640;
            let modal_h = 510;
            let filtered_catalog = state.launcher_state.filtered_catalog();
            let selected_idx = state.launcher_state.selected_index;
            let row_start_y = modal_y + 92;

            // 1. Render Catalog Items (Front of modal)
            for (i, app) in filtered_catalog.iter().enumerate().take(6) {
                let row_y = row_start_y + (i as i32 * 58);
                let is_selected = i == selected_idx;

                // Row Text Badge (Frontmost of the row)
                let label = format!("[{}] {}  •  {}", i + 1, app.title, app.description);
                let (row_bg_color, text_color) = if is_selected {
                    (active_theme_rgba, [255, 255, 255, 255])
                } else {
                    ([35, 42, 54, 220], [220, 225, 235, 255])
                };

                let row_badge = self.badge_renderer.get_or_create_badge(&label, row_bg_color, text_color);
                if let Ok(elem) = MemoryRenderBufferRenderElement::from_buffer(
                    renderer,
                    Point::from(((modal_x + 26) as f64, (row_y + 12) as f64)),
                    &row_badge,
                    None,
                    None,
                    None,
                    Kind::Unspecified,
                ) {
                    all_elements.push(KnotRenderElement::Memory(elem));
                }

                // Row Background (Behind row text)
                if is_selected {
                    let highlight_bg = Color32F::new(0.12, 0.16, 0.22, 0.98);
                    all_elements.push(Self::create_rect(modal_x + 16, row_y + 2, modal_w - 32, 46, highlight_bg));
                    all_elements.push(Self::create_rect(modal_x + 14, row_y, modal_w - 28, 50, active_theme_color));
                } else {
                    let row_bg = Color32F::new(0.10, 0.12, 0.16, 0.85);
                    all_elements.push(Self::create_rect(modal_x + 16, row_y + 2, modal_w - 32, 46, row_bg));
                }
            }

            // 2. Interactive Search Box Bar
            let query_text = if state.launcher_state.query.is_empty() {
                "SEARCH: [Type to filter / run command...]".to_string()
            } else {
                format!("SEARCH: {}_", state.launcher_state.query)
            };
            let search_badge = self.badge_renderer.get_or_create_badge(
                &query_text,
                [18, 24, 32, 250],
                if state.launcher_state.query.is_empty() { [140, 150, 170, 255] } else { [255, 255, 255, 255] },
            );
            if let Ok(elem) = MemoryRenderBufferRenderElement::from_buffer(
                renderer,
                Point::from(((modal_x + 24) as f64, (modal_y + 52) as f64)),
                &search_badge,
                None,
                None,
                None,
                Kind::Unspecified,
            ) {
                all_elements.push(KnotRenderElement::Memory(elem));
            }
            // Search box container background
            let search_bg = Color32F::new(0.06, 0.08, 0.11, 0.98);
            all_elements.push(Self::create_rect(modal_x + 16, modal_y + 46, modal_w - 32, 36, search_bg));
            all_elements.push(Self::create_rect(modal_x + 14, modal_y + 44, modal_w - 28, 40, active_theme_color));

            // 3. Header Banner Badge
            let header_badge = self.badge_renderer.get_or_create_badge(
                "🌟 KNOT APP LAUNCHER  •  [TYPE TO SEARCH]  •  [ESC TO CLOSE]",
                active_theme_rgba,
                [255, 255, 255, 255],
            );
            if let Ok(elem) = MemoryRenderBufferRenderElement::from_buffer(
                renderer,
                Point::from(((modal_x + 16) as f64, (modal_y + 14) as f64)),
                &header_badge,
                None,
                None,
                None,
                Kind::Unspecified,
            ) {
                all_elements.push(KnotRenderElement::Memory(elem));
            }

            // 4. Modal Dark Card Background (Behind content)
            let modal_bg = Color32F::new(0.086, 0.106, 0.133, 0.97);
            all_elements.push(Self::create_rect(modal_x, modal_y, modal_w, modal_h, modal_bg));

            // 5. Modal Outer Glow Border (Backmost of modal)
            all_elements.push(Self::create_rect(modal_x - 3, modal_y - 3, modal_w + 6, modal_h + 6, active_theme_color));
        }

        // =========================================================================
        // LAYER 3 (OVERLAY): Top Center HUD Pills (Active Seat & Launch App Button)
        // =========================================================================
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
            Point::from((460.0, 14.0)),
            &hud_badge,
            None,
            None,
            None,
            Kind::Unspecified,
        ) {
            all_elements.push(KnotRenderElement::Memory(elem));
        }

        // Top HUD Clickable Launcher Button
        let launcher_btn_badge = self.badge_renderer.get_or_create_badge(
            "🚀 [+ LAUNCH APP (Ctrl+Space)]",
            [40, 167, 69, 240], // Vibrant Green
            [255, 255, 255, 255],
        );
        if let Ok(elem) = MemoryRenderBufferRenderElement::from_buffer(
            renderer,
            Point::from((660.0, 14.0)),
            &launcher_btn_badge,
            None,
            None,
            None,
            Kind::Unspecified,
        ) {
            all_elements.push(KnotRenderElement::Memory(elem));
        }

        // =========================================================================
        // LAYER 4 (MIDDLE): Glowing Multi-Seat Focus Borders & Window Titles
        // =========================================================================
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

                    // Render Window Owner Badge (Front of border)
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

                    // 4 Border Glow Bars (Hollow outline)
                    all_elements.push(Self::create_rect(
                        x - border_thick,
                        y - border_thick,
                        w + border_thick * 2,
                        border_thick,
                        color,
                    ));
                    all_elements.push(Self::create_rect(
                        x - border_thick,
                        y + h,
                        w + border_thick * 2,
                        border_thick,
                        color,
                    ));
                    all_elements.push(Self::create_rect(
                        x - border_thick,
                        y,
                        border_thick,
                        h,
                        color,
                    ));
                    all_elements.push(Self::create_rect(
                        x + w,
                        y,
                        border_thick,
                        h,
                        color,
                    ));
                }
            }
        }

        // =========================================================================
        // LAYER 5 (BACK): Window Surfaces from Space
        // =========================================================================
        for elem in space_elements {
            all_elements.push(KnotRenderElement::Space(elem));
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
