use knot::render::text::BadgeTextRenderer;

#[test]
fn test_badge_renderer_creates_valid_buffer() {
    let bg_color = [255, 64, 129, 255]; // Pink
    let fg_color = [255, 255, 255, 255]; // White

    let (width, height, pixels) = BadgeTextRenderer::render_badge_pixels("ALICE", bg_color, fg_color);

    // Verify buffer geometry: height is 23 (13 font + 10 padding), width includes padding
    assert_eq!(height, 23);
    assert_eq!(width, (5 * 8 + 20) as i32); // 60px

    // Verify buffer contains non-empty pixel data
    assert!(!pixels.is_empty());
    assert_eq!(pixels.len(), (width * height * 4) as usize);

    // Verify alpha channel has fully opaque pixels
    let has_opaque_pixel = pixels.chunks_exact(4).any(|p| p[3] > 0);
    assert!(has_opaque_pixel, "Badge buffer must contain rendered visible pixels");
}

#[test]
fn test_badge_dimensions_calculation() {
    let (w_alice, h_alice) = BadgeTextRenderer::calculate_dimensions("ALICE");
    assert_eq!(w_alice, (5 * 8 + 20) as i32);
    assert_eq!(h_alice, 23);

    let (w_hud, h_hud) = BadgeTextRenderer::calculate_dimensions("ACTIVE: ALICE [TAB]");
    assert_eq!(w_hud, (19 * 8 + 20) as i32);
    assert_eq!(h_hud, 23);
}

#[test]
fn test_badge_caching() {
    let mut renderer = BadgeTextRenderer::new();

    let bg_color = [0, 229, 255, 255]; // Cyan
    let fg_color = [255, 255, 255, 255];

    let _badge1 = renderer.get_or_create_badge("BOB", bg_color, fg_color);
    let _badge2 = renderer.get_or_create_badge("BOB", bg_color, fg_color);
}
