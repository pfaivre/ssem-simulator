use std::path::Path;
use std::sync::Arc;

use crate::ssem::opcode::Opcode;

use super::{TempFile, instr, store_from_bits, word_bits};
use crate::ssem::store::{Bits, Store};

/// Write the given content to a uniquely named temporary file
fn temp_file(name: &str, content: &str) -> TempFile {
    let file = TempFile::new(name);
    std::fs::write(file.path(), content).expect("Unable to write the temporary test file");
    file
}

#[test]
fn bits_display_renders_bits_as_dots_and_hashes() {
    // Least significant bit first, like the SSEM displayed its words
    assert_eq!(format!("#{}", ".".repeat(31)), Bits(1).to_string());
    assert_eq!("#".repeat(32), Bits(-1).to_string());
    assert_eq!(format!("#.#{}", ".".repeat(29)), Bits(5).to_string());
}

#[test]
fn decode_instruction() {
    let store = Store::from(store_from_bits(&[
        (0, "10000111111110001111111111111111"),
        (1, "01000111111111001111111111111111"),
        (2, "11000111111110101111111111111111"),
        (3, "00100111111111101111111111111111"),
        (4, "10100111111110011111111111111111"),
        (5, "01100111111111011111111111111111"),
        (6, "11100111111110111111111111111111"),
        (7, "00010111111111111111111111111111"),
    ]));

    let expected_outputs = [
        (0, Opcode::JMP, 1),
        (1, Opcode::JRP, 2),
        (2, Opcode::LDN, 3),
        (3, Opcode::STO, 4),
        (4, Opcode::SUB, 5),
        (5, Opcode::SUB2, 6),
        (6, Opcode::CMP, 7),
        (7, Opcode::STP, 8),
    ];

    for (address, exp_opcode, exp_data) in expected_outputs.iter() {
        let (opcode, data) = store.decode_instruction(*address);
        assert_eq!(*exp_opcode, opcode);
        assert_eq!(*exp_data, data);
    }
}

#[test]
#[should_panic]
fn decode_instruction_error() {
    let store = Store::new();
    let _ = store.decode_instruction(32);
}

#[test]
fn eq() {
    // default stores
    let s1 = Store::new();
    let s2 = Store::new();
    assert!(s1 == s2);

    // with data
    let s1 = Store::from((0..32).map(word_bits).collect::<Vec<_>>());
    let s2 = Store::from((0..32).map(word_bits).collect::<Vec<_>>());
    assert!(s1 == s2);

    // One word difference
    let s1 = Store::from((0..32).map(word_bits).collect::<Vec<_>>());
    let mut s2_words: Vec<String> = (0..32).map(word_bits).collect();
    s2_words[31] = "11111000000000000000000000000001".into();
    let s2 = Store::from(s2_words);
    assert!(s1 != s2);
}

#[test]
fn index_i32() {
    let sample_store = store_from_bits(&[
        (1, "10000000000000000000000000000000"),
        (2, "01000000000000000000000000000000"),
        (3, "11000000000000000000000000000000"),
    ]);
    let store = Store::from(sample_store);

    assert_eq!(1, store[1]);
    assert_eq!(2, store[2]);
    assert_eq!(3, store[3]);
}

#[test]
#[should_panic]
fn index_i32_error() {
    let sample_store = store_from_bits(&[
        (1, "10000000000000000000000000000000"),
        (2, "01000000000000000000000000000000"),
        (3, "11000000000000000000000000000000"),
    ]);
    let store = Store::from(sample_store);

    let _ = store[32];
}

#[test]
#[should_panic(expected = "Unable to convert integer -1 to usize")]
fn index_negative_error() {
    let store = Store::new();
    let _ = store[-1];
}

#[test]
fn from_asm_file_instructions() {
    let file = temp_file(
        "valid.asm",
        "; a leading comment\n\n00 JMP 5 ; jump somewhere\n01 CMP\n02 NUM -3\n03 STO 27\n",
    );
    let store = Store::from_asm_file(file.path()).unwrap();

    assert_eq!(instr(Opcode::JMP, 5), store.words[0]);
    assert_eq!(instr(Opcode::CMP, 0), store.words[1]); // missing operand defaults to 0
    assert_eq!(-3, store.words[2]); // NUM stores the raw value
    assert_eq!(instr(Opcode::STO, 27), store.words[3]);
    assert_eq!(0, store.words[4]);
}

#[test]
fn from_asm_file_keeps_comments() {
    let file = temp_file(
        "comments.asm",
        "; a leading comment\n\n00 JMP 5 ; jump somewhere\n01 CMP\n02 NUM -3 ; negative three\n03 STO 27 ;\n04 STO 27 ;   padded   \n",
    );
    let store = Store::from_asm_file(file.path()).unwrap();

    // The text after ';' is kept verbatim, leading indentation included
    assert_eq!(Some(" jump somewhere".to_string()), store.comments[0]);
    assert_eq!(None, store.comments[1], "no comment on this line");
    assert_eq!(Some(" negative three".to_string()), store.comments[2]);
    assert_eq!(
        None, store.comments[3],
        "empty comment after ';' is dropped"
    );
    assert_eq!(
        Some("   padded".to_string()),
        store.comments[4],
        "indentation after ';' is kept, trailing whitespace dropped"
    );
    assert_eq!(None, store.comments[5], "untouched line has no comment");
    // A comment-only line does not attach to any word
    assert!(
        store
            .comments
            .iter()
            .all(|c| c.as_deref() != Some(" a leading comment"))
    );
}

#[test]
fn from_asm_file_factorct_keeps_inline_comments() {
    let store = Store::from_asm_file(Path::new("samples/factorct.asm")).unwrap();
    assert_eq!(None, store.comments[0], "line 00's comment is empty");
    assert_eq!(Some(" A = -a".to_string()), store.comments[1]);
    // Line 02's comment is indented further in the source: kept as-is
    assert_eq!(
        Some("   26 = -a  (i.e. -b, with b = a initially)".to_string()),
        store.comments[2]
    );
    assert_eq!(
        Some(" b: candidate factor; holds the answer when stopped".to_string()),
        store.comments[27]
    );
}

#[test]
fn from_snp_file_keeps_comments() {
    let file = temp_file(
        "comments.snp",
        "; a leading comment\n0000: 10000000000000000000000000000000 ; first word\n0001: 00000000000000000000000000000000\n",
    );
    let store = Store::from_snp_file(file.path()).unwrap();

    assert_eq!(Some(" first word".to_string()), store.comments[0]);
    assert_eq!(None, store.comments[1]);
}

#[test]
fn display_appends_comments() {
    let file = temp_file(
        "display_comments.asm",
        "00 JMP 5 ; jump somewhere\n01 STP ;   halt now\n",
    );
    let store = Store::from_asm_file(file.path()).unwrap();
    let text = store.to_string();
    let mut lines = text.lines();
    assert!(
        lines.next().unwrap().ends_with("JMP 05  ; jump somewhere"),
        "comment shown after the instruction"
    );
    assert!(
        lines.next().unwrap().ends_with("STP 00  ;   halt now"),
        "the comment's indentation is preserved"
    );
}

#[test]
fn from_asm_file_missing_file() {
    let err = Store::from_asm_file(Path::new("samples/does_not_exist.asm")).unwrap_err();
    assert!(
        err.to_string().contains("Error while reading"),
        "err: {err}"
    );
}

#[test]
fn from_asm_file_invalid_instruction() {
    let file = temp_file("invalid_instruction.asm", "00\n");
    let err = Store::from_asm_file(file.path()).unwrap_err();
    assert!(
        err.to_string().contains("invalid instruction '00'"),
        "err: {err}"
    );
}

#[test]
fn from_asm_file_invalid_index() {
    let file = temp_file("invalid_index.asm", "not_a_number JMP 0\n");
    let err = Store::from_asm_file(file.path()).unwrap_err();
    assert!(
        err.to_string().contains("Unable to read the number"),
        "err: {err}"
    );
}

#[test]
fn from_asm_file_invalid_operand() {
    let file = temp_file("invalid_operand.asm", "00 JMP not_a_number\n");
    let err = Store::from_asm_file(file.path()).unwrap_err();
    assert!(
        err.to_string().contains("Unable to read the number"),
        "err: {err}"
    );
}

#[test]
fn from_asm_file_operand_too_big() {
    let file = temp_file("operand_too_big.asm", "00 JMP 32\n");
    let err = Store::from_asm_file(file.path()).unwrap_err();
    assert!(
        err.to_string().contains("operand '32' non valid"),
        "err: {err}"
    );
}

#[test]
fn from_asm_file_operand_negative() {
    let file = temp_file("operand_negative.asm", "00 LDN -1\n");
    let err = Store::from_asm_file(file.path()).unwrap_err();
    assert!(
        err.to_string().contains("operand '-1' non valid"),
        "err: {err}"
    );
}

#[test]
fn from_asm_file_non_contiguous_lines() {
    let file = temp_file("non_contiguous.asm", "00 JMP 0\n01 LDN 24\n03 STO 26\n");
    let err = Store::from_asm_file(file.path()).unwrap_err();
    assert!(err.to_string().contains("expected index '2'"), "err: {err}");
}

#[test]
fn from_asm_file_index_out_of_store() {
    let mut content = String::new();
    for i in 0..32 {
        content.push_str(&format!("{i:02} NUM {i}\n"));
    }
    content.push_str("32 NUM 32\n");
    let file = temp_file("out_of_store.asm", &content);
    let err = Store::from_asm_file(file.path()).unwrap_err();
    assert!(
        err.to_string()
            .contains("Index '32' is bigger than the machine size (32)"),
        "err: {err}"
    );
}

#[test]
fn from_asm_file_invalid_opcode() {
    let file = temp_file("invalid_opcode.asm", "00 FOO 1\n");
    let err = Store::from_asm_file(file.path()).unwrap_err();
    assert!(
        err.to_string().contains("opcode 'FOO' non valid"),
        "err: {err}"
    );
}

#[test]
fn from_snp_file_words() {
    let file = temp_file(
        "valid.snp",
        "; a leading comment\n\n0000: 10000000000000000000000000000000 ; some comment\n0001: 11111111111111111111111111111111\n",
    );
    let store = Store::from_snp_file(file.path()).unwrap();

    assert_eq!(1, store.words[0]);
    assert_eq!(-1, store.words[1]);
    assert_eq!(0, store.words[2]);
}

#[test]
fn from_snp_file_missing_file() {
    let err = Store::from_snp_file(Path::new("samples/does_not_exist.snp")).unwrap_err();
    assert!(
        err.to_string().contains("Error while reading"),
        "err: {err}"
    );
}

#[test]
fn from_snp_file_invalid_syntax() {
    let file = temp_file(
        "invalid_syntax.snp",
        "0000 00000000000000000000000000000000\n",
    );
    let err = Store::from_snp_file(file.path()).unwrap_err();
    assert!(err.to_string().contains("invalid syntax"), "err: {err}");
}

#[test]
fn from_snp_file_invalid_index() {
    let file = temp_file(
        "invalid_index.snp",
        "zero: 00000000000000000000000000000000\n",
    );
    let err = Store::from_snp_file(file.path()).unwrap_err();
    assert!(
        err.to_string().contains("Unable to read the number"),
        "err: {err}"
    );
}

#[test]
fn from_snp_file_non_contiguous_lines() {
    let file = temp_file(
        "non_contiguous.snp",
        "0000: 00000000000000000000000000000000\n0002: 00000000000000000000000000000000\n",
    );
    let err = Store::from_snp_file(file.path()).unwrap_err();
    assert!(err.to_string().contains("expected index '1'"), "err: {err}");
}

#[test]
fn from_snp_file_index_out_of_store() {
    let mut content = String::new();
    for i in 0..32 {
        content.push_str(&format!("{:04}: 00000000000000000000000000000000\n", i));
    }
    content.push_str("0032: 00000000000000000000000000000000\n");
    let file = temp_file("out_of_store.snp", &content);
    let err = Store::from_snp_file(file.path()).unwrap_err();
    assert!(
        err.to_string()
            .contains("Index '32' is bigger than the machine size (32)"),
        "err: {err}"
    );
}

#[test]
fn from_snp_file_invalid_word_size() {
    let file = temp_file("invalid_word_size.snp", "0000: 10101010\n");
    let err = Store::from_snp_file(file.path()).unwrap_err();
    assert!(
        err.to_string()
            .contains("Invalid word size, expected 32, got 8"),
        "err: {err}"
    );
}

#[test]
fn from_snp_file_invalid_word() {
    let file = temp_file(
        "invalid_word.snp",
        "0000: 0000000000000000000000000000000X\n",
    );
    let err = Store::from_snp_file(file.path()).unwrap_err();
    assert!(
        err.to_string().contains("Unable to parse the word"),
        "err: {err}"
    );
}

#[test]
#[should_panic(expected = "Invalid store size. Expected 32, got 31")]
fn from_vec_wrong_store_size() {
    let words: Vec<String> = vec!["0".repeat(32); 31];
    let _ = Store::from(words);
}

#[test]
#[should_panic(expected = "Invalid word size")]
fn from_vec_wrong_word_size() {
    let mut words: Vec<String> = vec!["0".repeat(32); 32];
    words[0] = "0".repeat(31);
    let _ = Store::from(words);
}

#[test]
#[should_panic(expected = "Unable to parse the word")]
fn from_vec_invalid_word() {
    let mut words: Vec<String> = vec!["0".repeat(32); 32];
    words[0] = "X".repeat(32);
    let _ = Store::from(words);
}

#[test]
fn parse_word_number() {
    assert_eq!(Ok(42), Store::parse_word("42"));
    assert_eq!(Ok(-3), Store::parse_word("-3"));
    assert_eq!(Ok(0), Store::parse_word("0"));
    assert_eq!(Ok(i32::MAX), Store::parse_word(&i32::MAX.to_string()));
    assert_eq!(Ok(i32::MIN), Store::parse_word(&i32::MIN.to_string()));
    // Leading/trailing whitespace is trimmed
    assert_eq!(Ok(42), Store::parse_word("  42  "));
}

#[test]
fn parse_word_instruction() {
    assert_eq!(Ok(instr(Opcode::JMP, 5)), Store::parse_word("JMP 5"));
    assert_eq!(Ok(instr(Opcode::LDN, 24)), Store::parse_word("LDN 24"));
    assert_eq!(Ok(instr(Opcode::STO, 26)), Store::parse_word("STO 26"));
    assert_eq!(Ok(instr(Opcode::CMP, 0)), Store::parse_word("CMP"));
    assert_eq!(Ok(instr(Opcode::STP, 0)), Store::parse_word("STP"));
    assert_eq!(Ok(instr(Opcode::SUB, 27)), Store::parse_word("SUB 27"));
    assert_eq!(Ok(instr(Opcode::SUB2, 1)), Store::parse_word("SUB2 1"));
    assert_eq!(Ok(instr(Opcode::JRP, 20)), Store::parse_word("JRP 20"));
}

#[test]
fn parse_word_instruction_case_insensitive() {
    assert_eq!(Ok(instr(Opcode::LDN, 24)), Store::parse_word("ldn 24"));
    assert_eq!(Ok(instr(Opcode::STO, 26)), Store::parse_word("Sto 26"));
}

#[test]
fn parse_word_num() {
    assert_eq!(Ok(42), Store::parse_word("NUM 42"));
    assert_eq!(Ok(-3), Store::parse_word("NUM -3"));
    assert_eq!(Ok(0), Store::parse_word("NUM 0"));
}

#[test]
fn with_opcode_replaces_only_the_opcode_field() {
    let word = instr(Opcode::LDN, 24);
    assert_eq!(
        instr(Opcode::STO, 24),
        Store::with_opcode(word, Opcode::STO)
    );
    // A word with bits outside the opcode/data fields (a NUM value)
    // keeps them
    let raw = (0b101 << 16) | instr(Opcode::JMP, 3);
    let rewritten = Store::with_opcode(raw, Opcode::STP);
    assert_eq!(instr(Opcode::STP, 3) | (0b101 << 16), rewritten);
    assert_eq!(
        (Opcode::STP, 3),
        Store::decode_word(rewritten),
        "decode(with_opcode(w, op)) round-trips"
    );
}

#[test]
fn with_data_replaces_only_the_data_field() {
    let word = instr(Opcode::LDN, 24);
    assert_eq!(instr(Opcode::LDN, 7), Store::with_data(word, 7));
    // The data is masked to 5 bits, like decode_word reads it
    assert_eq!(instr(Opcode::LDN, 31), Store::with_data(word, 63));
    assert_eq!(instr(Opcode::LDN, 31), Store::with_data(word, -1));
    // Bits outside the two fields are preserved
    let raw = (0b101 << 16) | instr(Opcode::JMP, 3);
    assert_eq!(
        (0b101 << 16) | instr(Opcode::JMP, 12),
        Store::with_data(raw, 12)
    );
}

#[test]
fn parse_word_errors() {
    assert!(Store::parse_word("").is_err());
    assert!(Store::parse_word("   ").is_err());
    assert!(Store::parse_word("FOO 1").is_err());
    assert!(Store::parse_word("LDN 32").is_err());
    assert!(Store::parse_word("LDN -1").is_err());
    assert!(Store::parse_word("LDN abc").is_err());
    assert!(Store::parse_word("LDN 1 2").is_err());
    assert!(Store::parse_word("STO 99").is_err());
}

/// A store with one word of each kind: a plain instruction, an
/// instruction with a non-zero data field, and raw data words.
fn mixed_store() -> Store {
    let mut store = Store::new();
    store.words[0] = instr(Opcode::JMP, 0);
    store.words[1] = instr(Opcode::LDN, 24);
    store.words[2] = instr(Opcode::CMP, 5); // data bits set: still an instruction
    store.words[3] = -3; // data word
    store.words[4] = instr(Opcode::SUB, 7) | (1 << 20); // stray bit: not a plain instruction
    Arc::make_mut(&mut store.comments)[1] = Some(" A = -a".to_string());
    Arc::make_mut(&mut store.comments)[3] = Some("   negative three".to_string());
    store
}

#[test]
fn to_asm_string_writes_instructions_and_num_words() {
    let store = mixed_store();
    let text = store.to_asm_string();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(32, lines.len());
    assert_eq!("00 JMP 00", lines[0]);
    assert_eq!("01 LDN 24; A = -a", lines[1]);
    assert_eq!("02 CMP 05", lines[2]);
    assert_eq!("03 NUM -3;   negative three", lines[3]);
    // A word with a bit outside the opcode/data fields is saved as NUM
    assert_eq!(
        format!("04 NUM {}", instr(Opcode::SUB, 7) | (1 << 20)),
        lines[4]
    );
    // A zero word is a valid "JMP 00" instruction, not a NUM
    assert_eq!("05 JMP 00", lines[5]);
}

#[test]
fn to_asm_string_round_trips_through_from_asm_file() {
    let store = mixed_store();
    let file = temp_file("round_trip.asm", &store.to_asm_string());
    let reloaded = Store::from_asm_file(file.path()).unwrap();
    assert_eq!(store.words, reloaded.words);
    assert_eq!(store.comments, reloaded.comments);
}

#[test]
fn to_snp_string_writes_ssem_bit_order_and_comments() {
    let store = mixed_store();
    let text = store.to_snp_string();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(32, lines.len());
    // Least significant bit first: word 0 = JMP 0 = 0
    assert_eq!("0000: 00000000000000000000000000000000", lines[0]);
    // word 1 = LDN 24 = (2 << 13) | 24, bits reversed
    assert_eq!(
        format!("0001: {:032b}; A = -a", store.words[1].reverse_bits()),
        lines[1]
    );
    // word 3 = -3, all bits set except the two low ones
    assert_eq!(
        format!("0003: {:032b};   negative three", (-3i32).reverse_bits()),
        lines[3]
    );
}

#[test]
fn to_snp_string_round_trips_through_from_snp_file() {
    let store = mixed_store();
    let file = temp_file("round_trip.snp", &store.to_snp_string());
    let reloaded = Store::from_snp_file(file.path()).unwrap();
    assert_eq!(store.words, reloaded.words);
    assert_eq!(store.comments, reloaded.comments);
}

#[test]
fn save_picks_the_format_from_the_extension() {
    let store = mixed_store();
    let asm = temp_file("save.asm", "");
    store.save(asm.path()).unwrap();
    let content = std::fs::read_to_string(asm.path()).unwrap();
    assert!(content.starts_with("00 JMP 00\n"), "asm body: {content}");

    let snp = temp_file("save.snp", "");
    store.save(snp.path()).unwrap();
    let content = std::fs::read_to_string(snp.path()).unwrap();
    assert!(content.starts_with("0000: "), "snp body: {content}");

    // The extension check is case-insensitive
    let upper = temp_file("save.SNP", "");
    store.save(upper.path()).unwrap();
    assert!(
        std::fs::read_to_string(upper.path())
            .unwrap()
            .starts_with("0000: ")
    );
}

#[test]
fn save_rejects_missing_or_unknown_extensions() {
    let store = Store::new();
    let txt = temp_file("save.txt", "");
    let err = store.save(txt.path()).unwrap_err();
    assert_eq!(std::io::ErrorKind::InvalidInput, err.kind());
    assert!(err.to_string().contains(".asm or .snp"));
    assert!(!txt.path().exists() || std::fs::read_to_string(txt.path()).unwrap().is_empty());

    let no_ext = temp_file("save", "");
    assert!(store.save(no_ext.path()).is_err());
}

#[test]
fn save_reports_io_errors() {
    let store = Store::new();
    let err = store
        .save(Path::new("/nonexistent_dir_hopefully/save.snp"))
        .unwrap_err();
    assert_eq!(std::io::ErrorKind::NotFound, err.kind());
}
