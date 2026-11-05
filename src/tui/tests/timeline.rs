use super::*;

#[test]
pub(super) fn timeline_column_maps_to_cycle() {
    let mut sim = looping_sim(10);
    let mut app = App::new();
    app.timeline_area = Rect::new(0, 10, 80, 1);

    let (bar_x, bar_width) = timeline_bar_geometry(app.timeline_area, 0, 10);
    assert_eq!(10, bar_x);
    assert!(bar_width > 1);

    // Far right of the bar: the tip
    app.scrub_to_column(bar_x + bar_width - 1, &mut sim);
    assert_eq!(10, sim.cycle_count);

    // Left edge: the oldest retained cycle
    app.scrub_to_column(bar_x, &mut sim);
    assert_eq!(0, sim.cycle_count);

    // Middle
    app.scrub_to_column(bar_x + bar_width / 2, &mut sim);
    assert_eq!(5, sim.cycle_count);

    // Columns past the bar's ends clamp to the retained range,
    // so the marker (and the pop-up centered on it) stays on the bar
    app.scrub_to_column(bar_x + bar_width + 50, &mut sim);
    assert_eq!(10, sim.cycle_count);
    app.scrub_to_column(0, &mut sim);
    assert_eq!(0, sim.cycle_count);
    app.scrub_to_column(bar_x.saturating_sub(50), &mut sim);
    assert_eq!(0, sim.cycle_count);
}

#[test]
pub(super) fn scrub_relative_clamps_to_history_range() {
    let mut sim = looping_sim(10);
    let mut app = App::new();

    app.scrub_relative(-1000, &mut sim);
    assert_eq!(0, sim.cycle_count);
    assert!(sim.stop_flag, "scrubbing pauses the machine");

    app.scrub_relative(1000, &mut sim);
    assert_eq!(10, sim.cycle_count);

    // At the tip, stepping forward is a no-op and does not pause
    sim.stop_flag = false;
    app.scrub_relative(1, &mut sim);
    assert_eq!(10, sim.cycle_count);
    assert!(!sim.stop_flag);
}

#[test]
pub(super) fn scrub_keys_step_through_history() {
    let mut sim = looping_sim(10);
    let mut app = App::new();

    // Arrow keys step through the history, via the full key-event path
    app.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE), &mut sim);
    assert_eq!(9, sim.cycle_count);
    app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), &mut sim);
    assert_eq!(10, sim.cycle_count);
    app.handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE), &mut sim);
    assert_eq!(0, sim.cycle_count);
    app.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE), &mut sim);
    assert_eq!(10, sim.cycle_count);

    // The old , . keys no longer scrub
    app.handle_key(
        KeyEvent::new(KeyCode::Char(','), KeyModifiers::NONE),
        &mut sim,
    );
    app.handle_key(
        KeyEvent::new(KeyCode::Char('.'), KeyModifiers::NONE),
        &mut sim,
    );
    assert_eq!(10, sim.cycle_count);
}

#[test]
pub(super) fn scrubbing_keeps_the_heatmap_until_the_machine_restarts() {
    // An empty store executes `JMP 0` on line 1 forever
    let mut sim = looping_sim(10);
    assert_eq!(10, sim.exec_counts[1]);

    let mut app = App::new();
    app.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE), &mut sim);
    assert_eq!(9, sim.cycle_count);
    assert_eq!(10, sim.exec_counts[1], "scrubbing keeps the heatmap values");
    app.handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE), &mut sim);
    assert_eq!(0, sim.cycle_count);
    assert_eq!(10, sim.exec_counts[1], "still kept at the oldest cycle");

    // Restarting the machine from the rewound state resets the heatmap:
    // only the re-executed cycle is counted
    sim.instruction_cycle();
    assert_eq!(1, sim.exec_counts[1]);
}

#[test]
pub(super) fn editing_a_word_clears_the_history_to_the_edited_state() {
    let mut sim = looping_sim(10); // history 0..=10
    let mut app = App::new();

    // Open the word editor (E) and set word 7 to 42
    app.handle_key(
        KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE),
        &mut sim,
    );
    assert!(app.editor_popup.is_open());
    for code in [
        KeyCode::Char('7'),
        KeyCode::Enter,
        KeyCode::Char('4'),
        KeyCode::Char('2'),
        KeyCode::Enter,
    ] {
        app.handle_key(KeyEvent::new(code, KeyModifiers::NONE), &mut sim);
    }

    assert_eq!(42, sim.store.words[7]);
    let h = sim.history.as_ref().unwrap();
    assert_eq!(1, h.len());
    assert_eq!(10, h.start(), "the earliest state is the edited one");
    assert_eq!(10, h.head());
    assert_eq!(42, h.get(10).unwrap().store[7]);
    assert_eq!(None, h.get(9), "the pre-edit history is gone");

    // Scrubbing back cannot go before the edit anymore
    app.handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE), &mut sim);
    assert_eq!(10, sim.cycle_count);

    // The machine keeps recording from the edited state
    sim.instruction_cycle();
    let h = sim.history.as_ref().unwrap();
    assert_eq!(2, h.len());
    assert_eq!(10, h.start());
    assert_eq!(11, h.head());
}

#[test]
pub(super) fn scrub_without_history_is_noop() {
    let mut sim = Simulator::new();
    sim.run(10);
    let mut app = App::new();

    app.handle_scrub_key(KeyCode::Left, &mut sim);
    assert_eq!(10, sim.cycle_count);
    assert!(!sim.stop_flag);
}

#[test]
pub(super) fn timeline_bar_is_centered_on_wide_terminals() {
    // 200 columns: the bar is capped at 100 cells, so the whole
    // " HISTORY [bar] 0..99" group (10 + 100 + 7 = 117 cells) is centered
    // with ratatui's rounding: offset 200/2 - 117/2 = 100 - 58 = 42,
    // bar at 42 + 10
    let timeline = Rect::new(0, 18, 200, 1);
    let (bar_x, bar_width) = timeline_bar_geometry(timeline, 0, 99);
    assert_eq!(100, bar_width);
    assert_eq!(42 + 10, bar_x);

    // The drawn line matches: "HISTORY" starts one cell after the group's
    // left edge (the prefix's leading space), and the right margin holds
    // the remaining cells
    let mut app = App::new();
    let snap = snapshot_with_history(42);
    let buffer = render(&mut app, &snap, 200, 20);
    let row = row_text(&buffer, 18); // the timeline row on a 20-row terminal
    assert_eq!(
        43,
        row.find("HISTORY").expect("history bar drawn"),
        "{row:?}"
    );
    // (chars, not bytes: the bar glyphs are multi-byte)
    assert_eq!(
        159,
        row.trim_end().chars().count(),
        "group ends at 42 + 117: {row:?}"
    );

    // Mouse hit-testing follows the centered bar: its right edge is the head
    let mut sim = looping_sim(100);
    let (start, head) = {
        let h = sim.history.as_ref().unwrap();
        (h.start(), h.head())
    };
    // (the bar geometry depends on the start..head text, so recompute it
    // with the sim's actual range)
    let (bar_x, bar_width) = timeline_bar_geometry(Rect::new(0, 18, 200, 1), start, head);
    app.timeline_area = Rect::new(0, 18, 200, 1);
    app.scrub_to_column(bar_x + bar_width - 1, &mut sim);
    assert_eq!(head, sim.cycle_count, "bar's right edge is the head");
    app.scrub_to_column(bar_x, &mut sim);
    assert_eq!(start, sim.cycle_count, "bar's left edge is the start");
}

#[test]
pub(super) fn scrub_popup_centers_on_the_bar_marker() {
    // On an 80-column terminal the bar fills the width (63 cells), so the
    // centering offset is zero and the bar spans x=10, width 63
    let timeline = Rect::new(0, 18, 80, 1);
    let (bar_x, bar_width) = timeline_bar_geometry(timeline, 0, 99);
    assert_eq!(10, bar_x);
    assert_eq!(63, bar_width);

    for cycle_count in [0, 42, 99] {
        let mut app = App::new();
        app.scrubbing = true;
        let snap = snapshot_with_history(cycle_count);
        let buffer = render(&mut app, &snap, 80, 20);

        // The timeline row is 18 on a 20-row terminal; the pop-up sits on row 17
        let row = row_text(&buffer, 17);
        let text = format!(" cycle {cycle_count} ");
        let pos = row
            .find(&text)
            .unwrap_or_else(|| panic!("pop-up missing at cycle {cycle_count}, row: {row:?}"));
        let center = pos as u16 + text.len() as u16 / 2;
        let marker = bar_x + timeline_marker_offset(bar_width, 0, 99, cycle_count).unwrap() as u16;
        assert_eq!(
            marker, center,
            "pop-up should center on the marker at cycle {cycle_count}"
        );
    }
}

#[test]
pub(super) fn scrub_popup_hidden_when_not_scrubbing() {
    let mut app = App::new();
    let snap = snapshot_with_history(42);
    let buffer = render(&mut app, &snap, 80, 20);

    let all: String = (0..buffer.area.height)
        .map(|y| row_text(&buffer, y))
        .collect();
    assert!(!all.contains(" cycle 42 "), "no pop-up when not scrubbing");
}

#[test]
pub(super) fn scrub_popup_stays_on_screen_at_the_bar_ends() {
    // Narrowest supported terminal (wider with the default pointer
    // arrows), pop-up at both ends of the bar
    for cycle_count in [0, 99] {
        let mut app = App::new();
        app.scrubbing = true;
        let snap = snapshot_with_history(cycle_count);
        let buffer = render(&mut app, &snap, MIN_WIDTH + ARROW_EXTRA_WIDTH, 20);

        let row = row_text(&buffer, 17);
        let text = format!(" cycle {cycle_count} ");
        let pos = row
            .find(&text)
            .unwrap_or_else(|| panic!("pop-up missing at cycle {cycle_count}, row: {row:?}"));
        assert!(
            pos + text.len() <= buffer.area.width as usize,
            "pop-up must stay on screen at cycle {cycle_count}"
        );
    }
}
