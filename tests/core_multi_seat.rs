use knot::seats::KnotSeatManager;
use smithay::{
    input::SeatState,
    reexports::wayland_server::Display,
    utils::Point,
};
use knot::state::KnotState;

#[test]
fn test_seat_manager_initialization() {
    let display = Display::<KnotState>::new().expect("Failed to create Wayland display");
    let dh = display.handle();
    let mut seat_state = SeatState::<KnotState>::new();

    let seat_manager = KnotSeatManager::new(&dh, &mut seat_state);

    // Verify dual user seats are initialized
    assert_eq!(seat_manager.users.len(), 2, "Knot must initialize exactly 2 multi-user seats");
    assert_eq!(seat_manager.alice().name, "Alice (Host)");
    assert_eq!(seat_manager.bob().name, "Bob (Guest)");

    // Verify independent Figma colors
    assert_eq!(seat_manager.alice().color_hex, "#FF4081", "Alice must have Pink focus color");
    assert_eq!(seat_manager.bob().color_hex, "#00E5FF", "Bob must have Cyan focus color");

    // Verify independent seat handles
    assert_eq!(seat_manager.alice().seat.name(), "seat-alice");
    assert_eq!(seat_manager.bob().seat.name(), "seat-bob");
}

#[test]
fn test_seat_toggle_active() {
    let display = Display::<KnotState>::new().expect("Failed to create Wayland display");
    let dh = display.handle();
    let mut seat_state = SeatState::<KnotState>::new();

    let mut seat_manager = KnotSeatManager::new(&dh, &mut seat_state);

    assert_eq!(seat_manager.active_user_index, 0, "Alice should be active by default");
    assert_eq!(seat_manager.current_user().name, "Alice (Host)");

    // Toggle with Tab
    seat_manager.toggle_active_user();
    assert_eq!(seat_manager.active_user_index, 1, "Bob should be active after toggle");
    assert_eq!(seat_manager.current_user().name, "Bob (Guest)");

    // Toggle back
    seat_manager.toggle_active_user();
    assert_eq!(seat_manager.active_user_index, 0, "Alice should be active again");
}

#[test]
fn test_seat_independent_spatial_locations() {
    let display = Display::<KnotState>::new().expect("Failed to create Wayland display");
    let dh = display.handle();
    let mut seat_state = SeatState::<KnotState>::new();

    let mut seat_manager = KnotSeatManager::new(&dh, &mut seat_state);

    // Move Alice to (100, 200)
    seat_manager.alice_mut().location = Point::from((100.0, 200.0));

    // Move Bob to (800, 600)
    seat_manager.bob_mut().location = Point::from((800.0, 600.0));

    assert_eq!(seat_manager.alice().location.x, 100.0);
    assert_eq!(seat_manager.alice().location.y, 200.0);

    assert_eq!(seat_manager.bob().location.x, 800.0);
    assert_eq!(seat_manager.bob().location.y, 600.0);
}
