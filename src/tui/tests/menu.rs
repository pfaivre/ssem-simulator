//! Tests for the menu bar (the top row), the settings pop-up it opens, and
//! the status bar (the bottom row).

use super::super::popups::SettingControl;
use super::*;
use crate::ssem::simulator::Breakpoint;

/// Send a character key press.
fn press(app: &mut App, sim: &mut Simulator, c: char) {
    app.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE), sim);
}

/// Send a left-button press at the given cell.
fn click(app: &mut App, sim: &mut Simulator, column: u16, row: u16) {
    app.handle_mouse(
        MouseEvent {
            column,
            row,
            kind: MouseEventKind::Down(MouseButton::Left),
            modifiers: KeyModifiers::NONE,
        },
        sim,
    );
}

/// Send a mouse move at the given cell.
fn move_to(app: &mut App, sim: &mut Simulator, column: u16, row: u16) {
    app.handle_mouse(
        MouseEvent {
            column,
            row,
            kind: MouseEventKind::Moved,
            modifiers: KeyModifiers::NONE,
        },
        sim,
    );
}

/// The center cell of the menu item for `action` (the app must have been
/// drawn once, so the item rects are recorded).
fn menu_item_cell(app: &App, action: MenuAction) -> (u16, u16) {
    let (rect, _) = app
        .menu_items
        .iter()
        .find(|(_, a)| *a == action)
        .expect("menu item drawn");
    (rect.x + rect.width / 2, rect.y)
}

/// A screen cell on which the settings control is drawn (the app must have
/// been drawn once), found by probing the hit-test over the whole screen.
fn control_cell(app: &App, control: SettingControl) -> (u16, u16) {
    for row in 0..40u16 {
        for col in 0..100u16 {
            if app.settings_popup.control_at(col, row) == Some(control) {
                return (col, row);
            }
        }
    }
    panic!("settings control {control:?} not found on screen");
}

/// The whole screen text, rows joined with newlines.
fn screen_text(buffer: &Buffer) -> String {
    (0..buffer.area.height)
        .map(|row| row_text(buffer, row))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
pub(super) fn menu_bar_lists_all_actions_on_the_top_row() {
    let snap = snapshot_with_history(0); // stop_flag = true, so the item reads "Run"
    let mut app = App::new();
    let buffer = render(&mut app, &snap, 100, 40);

    let top = row_text(&buffer, 0);
    // The buttons carry no brackets
    assert!(
        !top.contains('[') && !top.contains(']'),
        "menu bar: {top:?}"
    );
    for item in ["Clear", "Save", "Quit", "Run", "Step", "Settings", "Help"] {
        assert!(top.contains(item), "menu bar missing {item}: {top:?}");
    }

    // While the machine runs, the Run/Stop item reads "Stop"
    let mut snap = snapshot_with_history(0);
    snap.stop_flag = false;
    let buffer = render(&mut app, &snap, 100, 40);
    let top = row_text(&buffer, 0);
    assert!(top.contains("Stop"), "menu bar: {top:?}");
    assert!(!top.contains("Run"), "menu bar: {top:?}");
}

#[test]
pub(super) fn menu_clicks_run_the_actions() {
    let mut app = App::new();
    let mut sim = Simulator::new();
    sim.enable_history(100);
    sim.run(5); // fills the heatmap counters and the history
    sim.store.words[3] = 42;
    Arc::make_mut(&mut sim.store.comments)[3] = Some("a comment".into());
    sim.stop_flag = true;
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);

    // Clear zeroes the store, drops the comments, resets the heatmaps,
    // resets the cycle count and restarts the history at cycle 0
    let (col, row) = menu_item_cell(&app, MenuAction::Clear);
    click(&mut app, &mut sim, col, row);
    assert_eq!(0, sim.cycle_count, "cycle count reset");
    assert!(sim.store.words.iter().all(|&w| w == 0), "store cleared");
    assert!(
        sim.store.comments.iter().all(Option::is_none),
        "comments cleared"
    );
    assert!(
        sim.exec_counts.iter().all(|&c| c == 0),
        "exec heatmap reset"
    );
    assert!(
        sim.read_counts.iter().all(|&c| c == 0),
        "read heatmap reset"
    );
    assert!(
        sim.write_counts.iter().all(|&c| c == 0),
        "write heatmap reset"
    );
    let history = sim.history.as_ref().expect("history kept");
    assert_eq!(1, history.len(), "history restarted");
    assert_eq!(0, history.start(), "history bounds reset");
    assert_eq!(0, history.head(), "history bounds reset");
    assert!(
        history.get(0).unwrap().store.iter().all(|&w| w == 0),
        "history holds the cleared state"
    );

    // Run/Stop toggles the machine
    let (col, row) = menu_item_cell(&app, MenuAction::RunStop);
    click(&mut app, &mut sim, col, row);
    assert!(!sim.stop_flag, "menu Run starts the machine");
    click(&mut app, &mut sim, col, row);
    assert!(sim.stop_flag, "menu Stop pauses the machine");

    // Step pauses and executes exactly one instruction
    let cycles = sim.cycle_count;
    let (col, row) = menu_item_cell(&app, MenuAction::Step);
    click(&mut app, &mut sim, col, row);
    assert_eq!(cycles + 1, sim.cycle_count, "one cycle executed");
    assert!(sim.stop_flag, "step leaves the machine paused");

    // Save opens the save pop-up
    let (col, row) = menu_item_cell(&app, MenuAction::Save);
    click(&mut app, &mut sim, col, row);
    assert!(app.save_popup.is_open(), "save pop-up opened");
    app.save_popup.close();

    // Settings and Help toggle their pop-ups (a second click closes them)
    let (col, row) = menu_item_cell(&app, MenuAction::Settings);
    click(&mut app, &mut sim, col, row);
    assert!(app.settings_popup.is_open(), "settings pop-up opened");
    click(&mut app, &mut sim, col, row);
    assert!(
        !app.settings_popup.is_open(),
        "second click toggles it closed"
    );

    let (col, row) = menu_item_cell(&app, MenuAction::Help);
    click(&mut app, &mut sim, col, row);
    assert!(app.help_popup.is_open(), "help pop-up opened");
    click(&mut app, &mut sim, col, row);
    assert!(!app.help_popup.is_open(), "second click toggles it closed");

    // Quit sets the quit flag
    let (col, row) = menu_item_cell(&app, MenuAction::Quit);
    click(&mut app, &mut sim, col, row);
    assert!(app.should_quit);
}

#[test]
pub(super) fn menu_opening_a_popup_closes_the_others() {
    let mut app = App::new();
    let mut sim = Simulator::new();
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);

    let (col, row) = menu_item_cell(&app, MenuAction::Settings);
    click(&mut app, &mut sim, col, row);
    assert!(app.settings_popup.is_open());

    // Clicking Help closes the settings pop-up and opens the help one
    let (col, row) = menu_item_cell(&app, MenuAction::Help);
    click(&mut app, &mut sim, col, row);
    assert!(!app.settings_popup.is_open(), "settings closed");
    assert!(app.help_popup.is_open(), "help opened");
}

#[test]
pub(super) fn space_runs_and_stops_and_ctrl_q_quits() {
    let mut app = App::new();
    let mut sim = Simulator::new();
    sim.stop_flag = true;

    // Space toggles run/pause
    press(&mut app, &mut sim, ' ');
    assert!(!sim.stop_flag, "Space starts the machine");
    press(&mut app, &mut sim, ' ');
    assert!(sim.stop_flag, "Space pauses the machine");

    // P is not bound to run/pause anymore
    press(&mut app, &mut sim, 'p');
    assert!(sim.stop_flag, "P must not toggle run/pause");

    // Plain Q and Esc do not quit anymore
    press(&mut app, &mut sim, 'q');
    assert!(!app.should_quit, "plain Q must not quit");
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &mut sim);
    assert!(!app.should_quit, "Esc must not quit");

    // Ctrl+Q quits (Ctrl+other letters are ignored, not plain-letter actions)
    app.handle_key(
        KeyEvent::new(KeyCode::Char('i'), KeyModifiers::CONTROL),
        &mut sim,
    );
    assert_eq!(
        700, sim.speed,
        "Ctrl+I must not trigger the plain I (speed up) action"
    );
    app.handle_key(
        KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL),
        &mut sim,
    );
    assert!(app.should_quit, "Ctrl+Q quits");
}

#[test]
pub(super) fn menu_hover_tracks_the_item_under_the_mouse() {
    // The hover highlight itself is a theme color and intentionally not
    // asserted; this checks the hit-testing and the hover state.
    let mut app = App::new();
    let mut sim = Simulator::new();
    let snap = snapshot_with_history(0);
    let _ = render(&mut app, &snap, 100, 40);

    // Moving over an item records it as hovered
    let (col, row) = menu_item_cell(&app, MenuAction::Settings);
    move_to(&mut app, &mut sim, col, row);
    assert_eq!(Some(MenuAction::Settings), app.menu_hover);

    // Moving to another item switches the hover
    let (col, row) = menu_item_cell(&app, MenuAction::Help);
    move_to(&mut app, &mut sim, col, row);
    assert_eq!(Some(MenuAction::Help), app.menu_hover);

    // Moving off the items (same row) clears it
    move_to(&mut app, &mut sim, 99, 0);
    assert_eq!(None, app.menu_hover);

    // Moving down to the next line clears it too — the highlight must not
    // stick when the cursor leaves the menu bar
    let (col, row) = menu_item_cell(&app, MenuAction::Settings);
    move_to(&mut app, &mut sim, col, row);
    assert_eq!(Some(MenuAction::Settings), app.menu_hover);
    move_to(&mut app, &mut sim, col, row + 1);
    assert_eq!(
        None, app.menu_hover,
        "leaving the menu bar clears the hover"
    );
}

#[test]
pub(super) fn settings_popup_lists_all_controls() {
    let mut app = App::new();
    let sim = Simulator::new();
    let snap = Snapshot::from_sim(&sim);
    app.settings_popup.open();
    let buffer = render(&mut app, &snap, 100, 40);

    let all = screen_text(&buffer);
    for text in [
        " SETTINGS ",
        "Machine speed",
        "ips",
        "Heatmap mode",
        "Flow mode",
        "Comments",
        "Store bits",
        "Theme",
    ] {
        assert!(
            all.contains(text),
            "settings pop-up missing {text:?}:\n{all}"
        );
    }
    // The current values are shown: default theme, comments on, control flow
    assert!(all.contains("[Dark]"), "theme value: {all}");
    assert!(all.contains("[ON]"), "comments value: {all}");
    assert!(all.contains("[CONTROL]"), "flow value: {all}");
}

#[test]
pub(super) fn settings_popup_controls_are_clickable() {
    let mut app = App::new();
    let mut sim = Simulator::new();
    let snap = Snapshot::from_sim(&sim);
    app.settings_popup.open();
    let _ = render(&mut app, &snap, 100, 40);

    // Heatmap / flow / comments / bits / theme buttons cycle their setting
    let start = (
        app.heatmap,
        app.flow,
        app.show_comments,
        app.bit_representation,
        app.theme,
    );
    for control in [
        SettingControl::Heatmap,
        SettingControl::Flow,
        SettingControl::Comments,
        SettingControl::Bits,
        SettingControl::Theme,
    ] {
        let (col, row) = control_cell(&app, control);
        click(&mut app, &mut sim, col, row);
    }
    assert_ne!(
        start,
        (
            app.heatmap,
            app.flow,
            app.show_comments,
            app.bit_representation,
            app.theme
        ),
        "each control changed its setting"
    );
    assert_eq!(start.0.cycle(), app.heatmap);
    assert_eq!(start.1.cycle(), app.flow);
    assert_eq!(!start.2, app.show_comments);
    assert_eq!(
        (start.3 + 1) % BIT_REPRESENTATIONS.len(),
        app.bit_representation
    );
    assert_eq!(start.4.cycle(), app.theme);

    // The speed buttons adjust the target speed
    let speed = sim.speed;
    let (col, row) = control_cell(&app, SettingControl::SpeedUp);
    click(&mut app, &mut sim, col, row);
    assert!(sim.speed > speed, "speed up button works");
    let (col, row) = control_cell(&app, SettingControl::SpeedDown);
    click(&mut app, &mut sim, col, row);
    assert_eq!(speed, sim.speed, "speed down button works");
}

#[test]
pub(super) fn settings_popup_keys_adjust_the_same_settings_and_esc_closes() {
    let mut app = App::new();
    let mut sim = Simulator::new();
    app.settings_popup.open();
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);

    let heatmap = app.heatmap;
    press(&mut app, &mut sim, 'm');
    assert_eq!(heatmap.cycle(), app.heatmap);

    let flow = app.flow;
    press(&mut app, &mut sim, 'f');
    assert_eq!(flow.cycle(), app.flow);

    let comments = app.show_comments;
    press(&mut app, &mut sim, 'c');
    assert_eq!(!comments, app.show_comments);

    let bits = app.bit_representation;
    press(&mut app, &mut sim, 'd');
    assert_eq!(
        (bits + 1) % BIT_REPRESENTATIONS.len(),
        app.bit_representation
    );

    let theme = app.theme;
    press(&mut app, &mut sim, 't');
    assert_eq!(theme.cycle(), app.theme);

    let speed = sim.speed;
    press(&mut app, &mut sim, 'i');
    assert!(sim.speed > speed, "I speeds up");
    press(&mut app, &mut sim, 'k');
    assert_eq!(speed, sim.speed, "K slows back down");

    // Esc closes the pop-up
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &mut sim);
    assert!(!app.settings_popup.is_open());
}

#[test]
pub(super) fn click_outside_the_settings_popup_closes_it() {
    let mut app = App::new();
    let mut sim = Simulator::new();
    let snap = Snapshot::from_sim(&sim);
    app.settings_popup.open();
    let _ = render(&mut app, &snap, 100, 40);

    // A click on the main body (outside the pop-up) closes it
    click(&mut app, &mut sim, 10, 10);
    assert!(!app.settings_popup.is_open());
}

#[test]
pub(super) fn settings_controls_are_highlighted_only_under_the_mouse() {
    let mut app = App::new();
    let mut sim = Simulator::new();
    let snap = Snapshot::from_sim(&sim);
    app.settings_popup.open();
    let buffer = render(&mut app, &snap, 100, 40);

    let heatmap_cell = control_cell(&app, SettingControl::Heatmap);
    let theme_cell = control_cell(&app, SettingControl::Theme);
    // The mouse is not over the pop-up yet: no control is highlighted
    assert_eq!(None, app.settings_popup.hover());
    for cell in [heatmap_cell, theme_cell] {
        assert!(
            !buffer[cell].modifier.contains(Modifier::REVERSED),
            "not highlighted before any hover"
        );
        assert_eq!(app.palette().bg, buffer[cell].bg);
    }

    // Hovering a control highlights only that one
    move_to(&mut app, &mut sim, heatmap_cell.0, heatmap_cell.1);
    assert_eq!(Some(SettingControl::Heatmap), app.settings_popup.hover());
    let buffer = render(&mut app, &snap, 100, 40);
    assert_eq!(app.palette().hover, buffer[heatmap_cell].bg);
    assert_eq!(app.palette().bg, buffer[theme_cell].bg);
    assert!(!buffer[theme_cell].modifier.contains(Modifier::REVERSED));

    // Moving to another control moves the highlight with the cursor
    move_to(&mut app, &mut sim, theme_cell.0, theme_cell.1);
    assert_eq!(Some(SettingControl::Theme), app.settings_popup.hover());
    let buffer = render(&mut app, &snap, 100, 40);
    assert_eq!(app.palette().hover, buffer[theme_cell].bg);
    assert_eq!(app.palette().bg, buffer[heatmap_cell].bg);

    // Moving inside the pop-up but off every control clears the highlight
    let off_control = (1..40u16)
        .flat_map(|row| (0..100u16).map(move |col| (col, row)))
        .find(|&(col, row)| {
            app.settings_popup.contains(col, row)
                && app.settings_popup.control_at(col, row).is_none()
        })
        .expect("a cell inside the pop-up but off the controls");
    move_to(&mut app, &mut sim, off_control.0, off_control.1);
    assert_eq!(None, app.settings_popup.hover());
    let buffer = render(&mut app, &snap, 100, 40);
    assert_eq!(app.palette().bg, buffer[heatmap_cell].bg);
    assert_eq!(app.palette().bg, buffer[theme_cell].bg);

    // Moving out of the pop-up clears it too
    move_to(&mut app, &mut sim, heatmap_cell.0, heatmap_cell.1);
    assert_eq!(Some(SettingControl::Heatmap), app.settings_popup.hover());
    let outside = (1..40u16)
        .flat_map(|row| (0..100u16).map(move |col| (col, row)))
        .find(|&(col, row)| !app.settings_popup.contains(col, row))
        .expect("a cell outside the pop-up");
    move_to(&mut app, &mut sim, outside.0, outside.1);
    assert_eq!(None, app.settings_popup.hover());
    let buffer = render(&mut app, &snap, 100, 40);
    assert_eq!(app.palette().bg, buffer[heatmap_cell].bg);
}

#[test]
pub(super) fn settings_hover_is_cleared_when_the_popup_closes() {
    let mut app = App::new();
    let mut sim = Simulator::new();
    let snap = Snapshot::from_sim(&sim);
    app.settings_popup.open();
    let _ = render(&mut app, &snap, 100, 40);

    let (col, row) = control_cell(&app, SettingControl::Theme);
    move_to(&mut app, &mut sim, col, row);
    assert_eq!(Some(SettingControl::Theme), app.settings_popup.hover());

    // Esc closes the pop-up and forgets the hover
    press(&mut app, &mut sim, 'h'); // routed nowhere: settings only takes mode keys
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &mut sim);
    assert!(!app.settings_popup.is_open());
    assert_eq!(None, app.settings_popup.hover());

    // Reopening starts with no highlight
    app.settings_popup.open();
    assert_eq!(None, app.settings_popup.hover());
}

#[test]
pub(super) fn status_bar_shows_left_and_right_groups_on_the_bottom_row() {
    let mut store = [0; 32];
    store[2] = Store::parse_word("SUB 10").unwrap();
    let mut snap = pointer_snapshot(2, store);
    snap.breakpoints.push(Breakpoint::CiValue {
        address: 5,
        was_met: false,
    });

    let mut app = App::new();
    app.heatmap = HeatmapMode::Exec;
    app.flow = FlowMode::Data;
    let buffer = render(&mut app, &snap, 120, 40);

    let status_row = buffer.area.height - 1;
    let row = row_text(&buffer, status_row);
    // The blocks carry no brackets
    assert!(
        !row.contains('[') && !row.contains(']'),
        "status bar: {row:?}"
    );
    // Left: status, cycles, last instruction, speed
    assert!(row.contains("STATUS: STOPPED"), "status: {row:?}");
    assert!(row.contains("CYCLES: 0"), "cycles: {row:?}");
    assert!(row.contains("02 SUB 10"), "last instruction: {row:?}");
    assert!(row.contains("SPEED:"), "speed: {row:?}");
    assert!(row.contains("/700 ips"), "target speed: {row:?}");
    // Right: heatmap, flow, comments, breakpoints, flush to the right edge
    for block in ["HEATMAP: EXEC", "FLOW: DATA", "COMMENTS", "BREAKPOINTS: 1"] {
        assert!(row.contains(block), "right side missing {block}: {row:?}");
    }
    assert!(
        row.trim_end().ends_with("BREAKPOINTS: 1"),
        "right group flush to the right edge: {row:?}"
    );

    // The menu bar (top row) does not carry the status info anymore
    let top = row_text(&buffer, 0);
    assert!(!top.contains("STATUS"), "menu bar: {top:?}");
    assert!(!top.contains("CYCLES"), "menu bar: {top:?}");
}
