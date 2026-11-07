use super::*;

fn press(c: char, mods: KeyModifiers) -> KeyEvent {
    KeyEvent::new_with_kind(KeyCode::Char(c), mods, KeyEventKind::Press)
}

#[test]
fn breakpoint_popup_accepts_plain_and_azerty_shifted_digits() {
    let mut popup = BreakpointPopup::new();
    popup.open();
    let mut breakpoints = Vec::new();

    popup.handle_key(press('9', KeyModifiers::NONE), &mut breakpoints);
    // AZERTY top row with Shift, as CSI-u terminals deliver it
    // (unshifted char + SHIFT modifier)
    popup.handle_key(press('&', KeyModifiers::SHIFT), &mut breakpoints); // -> 1
    popup.handle_key(press('\u{e9}', KeyModifiers::SHIFT), &mut breakpoints); // é -> 2
    popup.handle_key(press('"', KeyModifiers::SHIFT), &mut breakpoints); // -> 3
    popup.handle_key(press('\'', KeyModifiers::SHIFT), &mut breakpoints); // -> 4
    popup.handle_key(press('(', KeyModifiers::SHIFT), &mut breakpoints); // -> 5
    popup.handle_key(press('-', KeyModifiers::SHIFT), &mut breakpoints); // -> 6
    popup.handle_key(press('\u{e8}', KeyModifiers::SHIFT), &mut breakpoints); // è -> 7
    popup.handle_key(press('_', KeyModifiers::SHIFT), &mut breakpoints); // -> 8
    popup.handle_key(press('\u{e7}', KeyModifiers::SHIFT), &mut breakpoints); // ç -> 9
    popup.handle_key(press('\u{e0}', KeyModifiers::SHIFT), &mut breakpoints); // à -> 0
    assert_eq!("91234567890", popup.buffer); // 9 then AZERTY 1..0 (11-char cap fits)

    // Without Shift, the AZERTY chars are not digits
    popup.handle_key(press('&', KeyModifiers::NONE), &mut breakpoints);
    assert_eq!("91234567890", popup.buffer);
}

#[test]
fn breakpoint_popup_minus_still_works_without_shift() {
    let mut popup = BreakpointPopup::new();
    popup.open();
    let mut breakpoints = Vec::new();

    // Enter an address to move to the Value field
    popup.handle_key(press('3', KeyModifiers::NONE), &mut breakpoints);
    popup.handle_key(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        &mut breakpoints,
    );
    assert!(matches!(popup.field, InputField::Value));

    popup.handle_key(press('-', KeyModifiers::NONE), &mut breakpoints);
    assert_eq!("-", popup.buffer);
}

#[test]
fn editor_popup_accepts_azerty_shifted_digits() {
    let mut popup = EditorPopup::new();
    popup.open_for_ci();
    let mut sim = Simulator::new();

    popup.handle_key(press('\u{e0}', KeyModifiers::SHIFT), &mut sim); // à -> 0
    popup.handle_key(press('(', KeyModifiers::SHIFT), &mut sim); // ( -> 5
    popup.handle_key(press('7', KeyModifiers::NONE), &mut sim);
    assert_eq!("057", popup.buffer);
}

#[test]
fn editor_popup_rejects_out_of_range_ci() {
    let mut popup = EditorPopup::new();
    popup.open_for_ci();
    let mut sim = Simulator::new();

    // i32::MAX used to be stored unvalidated; the next fetch (`ci += 1`)
    // then panicked with an overflow in debug builds
    for c in "2147483647".chars() {
        popup.handle_key(press(c, KeyModifiers::NONE), &mut sim);
    }
    popup.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &mut sim);
    assert_eq!(0, sim.ci);
    assert_eq!(ts!("popup.address_range"), popup.message);

    // Upper boundary: 31 is accepted, 32 is rejected
    for c in "31".chars() {
        popup.handle_key(press(c, KeyModifiers::NONE), &mut sim);
    }
    popup.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &mut sim);
    assert_eq!(31, sim.ci);

    for c in "32".chars() {
        popup.handle_key(press(c, KeyModifiers::NONE), &mut sim);
    }
    popup.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &mut sim);
    assert_eq!(31, sim.ci, "32 is out of range and must not be applied");
    assert_eq!(ts!("popup.address_range"), popup.message);
}

#[test]
fn editor_popup_accepts_t_in_instruction_field() {
    let mut popup = EditorPopup::new();
    popup.open_for_word(5);
    let mut sim = Simulator::new();

    // A mnemonic containing T must be typable in the VALUE/INSTR field
    popup.handle_key(press('s', KeyModifiers::NONE), &mut sim);
    popup.handle_key(press('t', KeyModifiers::NONE), &mut sim);
    popup.handle_key(press('p', KeyModifiers::NONE), &mut sim);
    assert_eq!("STP", popup.buffer);
    assert!(matches!(popup.target, EditTarget::Word));

    // Enter applies it as the STP instruction
    popup.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &mut sim);
    assert_eq!(
        sim.store.words[5],
        Store::parse_word("STP").unwrap(),
        "message: {}",
        popup.message
    );
}

#[test]
fn editor_popup_t_still_cycles_target_outside_instruction_field() {
    let mut popup = EditorPopup::new();
    popup.open(); // WORD target, ADDRESS field
    let mut sim = Simulator::new();

    popup.handle_key(press('t', KeyModifiers::NONE), &mut sim);
    assert!(matches!(popup.target, EditTarget::Ci));
    popup.handle_key(press('T', KeyModifiers::NONE), &mut sim);
    assert!(matches!(popup.target, EditTarget::A));
    popup.handle_key(press('t', KeyModifiers::NONE), &mut sim);
    assert!(matches!(popup.target, EditTarget::Word));
    assert!(matches!(popup.field, EditField::Address));
}

#[test]
fn default_save_name_appends_edited_only_once() {
    assert_eq!(
        "factorct_edited.asm",
        default_save_name(Path::new("samples/factorct.asm"))
    );
    assert_eq!(
        "nightmare_edited.snp",
        default_save_name(Path::new("nightmare.snp"))
    );
    // Already edited: the suffix is not added again
    assert_eq!(
        "factorct_edited.asm",
        default_save_name(Path::new("/tmp/factorct_edited.asm"))
    );
    assert_eq!(
        "factorct_edited.snp",
        default_save_name(Path::new("factorct_edited.snp"))
    );
    // No extension: just the stem
    assert_eq!("state_edited", default_save_name(Path::new("state")));
    assert_eq!("_edited", default_save_name(Path::new("_edited")));
    // A stem merely containing "edited" (not the suffix) still gets it
    assert_eq!(
        "edited_edited.asm",
        default_save_name(Path::new("edited.asm"))
    );
}
