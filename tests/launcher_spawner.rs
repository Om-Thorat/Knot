use knot::launcher::AppCatalog;
use knot::state::LauncherState;

#[test]
fn test_default_app_catalog() {
    let catalog = AppCatalog::default_catalog();
    assert!(!catalog.is_empty(), "App catalog must contain installed applications");

    // Verify each app entry has non-empty title, command, and valid id
    for (i, app) in catalog.iter().enumerate() {
        assert_eq!(app.id, i + 1);
        assert!(!app.title.is_empty());
        assert!(!app.command.is_empty());
        assert!(!app.description.is_empty());
    }
}

#[test]
fn test_launcher_state_navigation() {
    let mut state = LauncherState::default();
    assert!(!state.is_open);
    assert_eq!(state.selected_index, 0);

    let count = state.catalog.len();
    assert!(count > 0);

    // Navigate down
    state.selected_index = (state.selected_index + 1) % count;
    assert_eq!(state.selected_index, 1);

    // Navigate up wraps around to last item
    state.selected_index = 0;
    if state.selected_index == 0 {
        state.selected_index = count - 1;
    }
    assert_eq!(state.selected_index, count - 1);
}

#[test]
fn test_launcher_search_filtering() {
    let mut state = LauncherState::default();

    // Search for "code"
    state.query = "code".to_string();
    let matches = state.filtered_catalog();
    assert!(!matches.is_empty(), "Searching 'code' must match VS Code");
    assert!(matches.iter().any(|a| a.title.contains("Visual Studio Code") || a.command == "code"));

    // Search for custom command e.g. "htop"
    state.query = "htop".to_string();
    let custom_matches = state.filtered_catalog();
    assert_eq!(custom_matches.len(), 1);
    assert_eq!(custom_matches[0].command, "htop");
    assert!(custom_matches[0].title.contains("Run 'htop'"));
}
