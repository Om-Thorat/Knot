use knot::island::multiplexer::SeatMultiplexer;
use smithay::backend::input::Keycode;

#[test]
fn test_multiplexer_default_state() {
    let multiplexer = SeatMultiplexer::new();
    assert_eq!(multiplexer.active_seat_name, "seat-alice");
    assert!(multiplexer.virtual_seat.is_none());
    assert!(multiplexer.virtual_pointer.is_none());
    assert!(multiplexer.virtual_keyboard.is_none());
}

#[test]
fn test_wayland_wire_to_xkb_keycode_mapping() {
    // Linux evdev keycodes from linux/input-event-codes.h
    let evdev_backspace = 14u32;
    let evdev_enter = 28u32;
    let evdev_space = 57u32;
    let evdev_key_a = 30u32;

    // Verify XKB translation (+8 offset)
    let xkb_backspace = Keycode::new(evdev_backspace + 8);
    let xkb_enter = Keycode::new(evdev_enter + 8);
    let xkb_space = Keycode::new(evdev_space + 8);
    let xkb_key_a = Keycode::new(evdev_key_a + 8);

    assert_eq!(xkb_backspace.raw(), 22, "Backspace in XKB space must be 22");
    assert_eq!(xkb_enter.raw(), 36, "Enter in XKB space must be 36");
    assert_eq!(xkb_space.raw(), 65, "Space in XKB space must be 65");
    assert_eq!(xkb_key_a.raw(), 38, "'A' key in XKB space must be 38");

    // Verify that subtracting 8 on the client side restores the exact evdev keycode
    assert_eq!(xkb_backspace.raw() - 8, evdev_backspace);
    assert_eq!(xkb_enter.raw() - 8, evdev_enter);
    assert_eq!(xkb_space.raw() - 8, evdev_space);
    assert_eq!(xkb_key_a.raw() - 8, evdev_key_a);
}
