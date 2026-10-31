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

    app.handle_scrub_key(KeyCode::Char(','), &mut sim);
    assert_eq!(9, sim.cycle_count);
    app.handle_scrub_key(KeyCode::Char('.'), &mut sim);
    assert_eq!(10, sim.cycle_count);
    app.handle_scrub_key(KeyCode::Home, &mut sim);
    assert_eq!(0, sim.cycle_count);
    app.handle_scrub_key(KeyCode::End, &mut sim);
    assert_eq!(10, sim.cycle_count);
}

#[test]
pub(super) fn scrub_without_history_is_noop() {
    let mut sim = Simulator::new();
    sim.run(10);
    let mut app = App::new();

    app.handle_scrub_key(KeyCode::Char(','), &mut sim);
    assert_eq!(10, sim.cycle_count);
    assert!(!sim.stop_flag);
}

#[test]
pub(super) fn scrub_popup_centers_on_the_bar_marker() {
    // On an 80-column terminal the bar spans x=10, width 63
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
