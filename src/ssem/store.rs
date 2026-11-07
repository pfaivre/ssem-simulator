use std::fmt;
use std::fmt::Write;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::ops::Index;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

use super::opcode::Opcode;

const ASM_COMMENT_CHAR: char = ';';
const SSEM_STORE_WORDS: usize = 32;
const SSEM_DATA_MASK: i32 = 0b00000000000000000000000000011111; // u5 equivalent
const SSEM_OPCODE_MASK: i32 = 0b00000000000000000000000000000111; // u3 equivalent
const SSEM_OPCODE_BIT_SHIFT: u8 = 13;

/// The on-disk format of a program/snapshot file, chosen by the file
/// extension: `.asm` for assembly, `.snp` for a raw binary snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FileFormat {
    Asm,
    Snp,
}

impl FileFormat {
    /// The format implied by the file name's extension (case-insensitive).
    ///
    /// Returns an `InvalidInput` error when the extension is missing or
    /// unknown, so a mistyped name never silently picks a format.
    pub(crate) fn from_path(path: &Path) -> std::io::Result<FileFormat> {
        match path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase())
            .as_deref()
        {
            Some("asm") => Ok(FileFormat::Asm),
            Some("snp") => Ok(FileFormat::Snp),
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                ts!("error.bad_extension", path = path.display().to_string()),
            )),
        }
    }

    /// Load a store from `path` in this format. This is the single
    /// extension→load-action dispatch: every caller goes through here, so a
    /// new format is one variant plus one arm here (and in `dump`).
    pub(crate) fn load(&self, path: &Path) -> std::io::Result<Store> {
        match self {
            FileFormat::Asm => Store::from_asm_file(path),
            FileFormat::Snp => Store::from_snp_file(path),
        }
    }

    /// Serialize `store` in this format (the body only, without the
    /// machine-state header comment `Simulator::save` prepends).
    pub(crate) fn dump(&self, store: &Store) -> String {
        match self {
            FileFormat::Asm => store.to_asm_string(),
            FileFormat::Snp => store.to_snp_string(),
        }
    }
}

/// An `InvalidData` io::Error carrying a localized parse message, so the
/// load path reports bad input as `Err` (like the save path) instead of
/// panicking.
fn parse_error(message: String) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, message)
}

/// Wrap a file-read failure with the localized `error.read_file` message,
/// preserving the original error kind.
fn read_error(path: &Path, e: std::io::Error) -> std::io::Error {
    let kind = e.kind();
    std::io::Error::new(
        kind,
        ts!(
            "error.read_file",
            path = path.display().to_string(),
            error = e
        ),
    )
}

/// Main memory of a SSEM-like machine
///
/// Note: SSEM writes integers with its least significant to most significant bits from left to right.
/// This is the opposite of modern computers, here we simply reverse the order during display.
#[derive(Debug)]
pub struct Store {
    pub words: [i32; SSEM_STORE_WORDS],

    /// The `;` comment found on each word's line in the input file, if any.
    /// Purely informational: shown in the displays, never executed. Shared
    /// behind an `Arc` so the TUI's per-frame snapshot can copy it with a
    /// refcount bump instead of cloning up to 32 strings every frame; the
    /// TUI edits a comment through `set_comment`, which swaps in a new `Arc`
    /// (`Arc::make_mut`) so in-flight snapshots keep the previous text.
    pub comments: Arc<[Option<String>; SSEM_STORE_WORDS]>,
}

impl Default for Store {
    fn default() -> Self {
        Self::new()
    }
}

impl Store {
    /// Mask that wraps any address into the store range.
    ///
    /// Valid because the store size is a power of two: for any i32,
    /// `address & INDEX_MASK` equals `address.rem_euclid(store size)`.
    pub const INDEX_MASK: i32 = (SSEM_STORE_WORDS - 1) as i32;

    /// Instanciate a new store.
    ///
    /// Every word gets initialized to zero.
    pub fn new() -> Store {
        let store = Store {
            words: [0_i32; SSEM_STORE_WORDS],
            comments: Arc::new(std::array::from_fn(|_| None)),
        };
        store._check();
        store
    }

    /// Zero every word and clear every source comment.
    pub fn clear(&mut self) {
        self.words = [0_i32; SSEM_STORE_WORDS];
        *Arc::make_mut(&mut self.comments) = std::array::from_fn(|_| None);
    }

    /// Initializes the store with the given assembly file
    ///
    /// An assembly file has the following form:
    /// ```
    /// 00 JMP 0  ;
    /// 01 LDN 24 ; -24 to C
    /// 02 STO 26 ; C to 26
    /// ...
    /// ```
    /// Each numbered line represents a word with its instruction. The text
    /// after ';' is kept as the word's comment (`Store::comments`) for
    /// display, with its original indentation.
    ///
    /// # Arguments
    ///
    /// * `filename` - Path to the file to read
    ///
    /// # Errors
    ///
    /// Returns an `io::Error` with a localized message when the file cannot be
    /// read or a line cannot be parsed, instead of panicking.
    pub fn from_asm_file(filename: &Path) -> std::io::Result<Store> {
        let file = File::open(filename).map_err(|e| read_error(filename, e))?;
        let reader = BufReader::new(file);

        let mut words = [0_i32; SSEM_STORE_WORDS];
        let mut comments = std::array::from_fn(|_| None);

        let mut last_index: i32 = 0;

        for line in reader.lines() {
            let line = line.map_err(|e| read_error(filename, e))?;

            // Split the instruction from its comment; the comment is kept
            // for display (a comment-only line has no instruction and is skipped)
            let (instruction, comment) = match line.split_once(ASM_COMMENT_CHAR) {
                Some((instruction, comment)) => {
                    // Keep the comment's original indentation (the spacing
                    // after the ';'), so aligned comments in the source stay
                    // aligned on screen; only trailing whitespace is dropped
                    let comment = comment.trim_end();
                    (
                        instruction.trim(),
                        (!comment.trim().is_empty()).then(|| comment.to_string()),
                    )
                }
                None => (line.trim(), None),
            };
            if instruction.is_empty() {
                continue;
            }
            // Extracting tokens "<index> <opcode> <operand>"
            let i: Vec<&str> = instruction.split_ascii_whitespace().collect();
            if i.len() < 2 {
                return Err(parse_error(ts!(
                    "error.invalid_instruction",
                    instruction = instruction
                )));
            }

            let index: i32 = i[0]
                .parse()
                .map_err(|_| parse_error(ts!("error.read_number")))?;
            let opcode: &str = i[1];

            let operand: i32 = if i.len() >= 3 {
                i[2].parse()
                    .map_err(|_| parse_error(ts!("error.read_number")))?
            } else {
                0
            };

            // Ensure lines are contiguous and without duplicates
            if index > 0 && index != last_index + 1 {
                return Err(parse_error(ts!(
                    "error.expected_index",
                    line = line,
                    expected = last_index + 1
                )));
            }
            last_index = index;

            // Ensure the index is within the store (this also rejects negatives,
            // which the contiguity check above lets through)
            if !(0..words.len() as i32).contains(&index) {
                return Err(parse_error(ts!(
                    "error.index_too_big",
                    line = instruction,
                    index = index,
                    size = words.len()
                )));
            }
            let addr = index as usize;

            comments[addr] = comment;

            let opcode = Opcode::from_str(opcode).map_err(|_| {
                parse_error(ts!(
                    "error.opcode_not_valid",
                    line = instruction,
                    opcode = opcode
                ))
            })?;

            match opcode {
                Opcode::NUM => {
                    words[addr] = operand;
                }
                opcode => {
                    // Ensure the operand fits in the instruction's data bits
                    if !(0..=SSEM_DATA_MASK).contains(&operand) {
                        return Err(parse_error(ts!(
                            "error.operand_not_valid",
                            line = instruction,
                            operand = operand,
                            max = SSEM_DATA_MASK
                        )));
                    }

                    // Print the opcode
                    let w: i32 = (opcode as i32) << SSEM_OPCODE_BIT_SHIFT;

                    // Print the operand
                    words[addr] = w | operand;
                }
            }
        }

        let store = Store {
            words,
            comments: Arc::new(comments),
        };
        store._check();

        Ok(store)
    }

    /// Initializes the store with the given snp file
    ///
    /// A snp file has the following form:
    /// ```
    /// 0000: 10000000000000000000000000000000
    /// 0001: 01010000000000100000000000000000 ; Some comment
    /// 0002: 00000000000000010000000000000000
    /// ...
    /// ```
    /// Each numbered line represents a raw word. The text after ';' is kept
    /// as the word's comment (`Store::comments`) for display, with its
    /// original indentation.
    ///
    /// # Arguments
    ///
    /// * `filename` - Path to the file to read
    ///
    /// # Errors
    ///
    /// Returns an `io::Error` with a localized message when the file cannot be
    /// read or a line cannot be parsed, instead of panicking.
    pub fn from_snp_file(filename: &Path) -> std::io::Result<Store> {
        let file = File::open(filename).map_err(|e| read_error(filename, e))?;
        let reader = BufReader::new(file);

        let mut words = [0_i32; SSEM_STORE_WORDS];
        let mut comments = std::array::from_fn(|_| None);

        let mut last_index: i32 = 0;

        for line in reader.lines() {
            let line = line.map_err(|e| read_error(filename, e))?;

            // Split the word from its comment; the comment is kept for
            // display (a comment-only line has no word and is skipped)
            let (instruction, comment) = match line.split_once(ASM_COMMENT_CHAR) {
                Some((instruction, comment)) => {
                    // Keep the comment's original indentation (the spacing
                    // after the ';'), so aligned comments in the source stay
                    // aligned on screen; only trailing whitespace is dropped
                    let comment = comment.trim_end();
                    (
                        instruction.trim(),
                        (!comment.trim().is_empty()).then(|| comment.to_string()),
                    )
                }
                None => (line.trim(), None),
            };
            if instruction.is_empty() {
                continue;
            }
            // Extracting tokens "<index>: <binary_word>"
            let i: Vec<&str> = instruction.splitn(2, ':').collect();
            if i.len() != 2 {
                return Err(parse_error(ts!("error.invalid_syntax", line = instruction)));
            }

            let index: i32 = i[0]
                .parse()
                .map_err(|_| parse_error(ts!("error.read_number")))?;

            // Ensure lines are contiguous and without duplicates
            if index > 0 && index != last_index + 1 {
                return Err(parse_error(ts!(
                    "error.expected_index",
                    line = line,
                    expected = last_index + 1
                )));
            }
            last_index = index;

            // Ensure the index is within the store (this also rejects negatives,
            // which the contiguity check above lets through)
            if !(0..words.len() as i32).contains(&index) {
                return Err(parse_error(ts!(
                    "error.index_too_big",
                    line = instruction,
                    index = index,
                    size = words.len()
                )));
            }
            let addr = index as usize;

            comments[addr] = comment;

            // Reverse the bit order: SSEM is least significant bit first
            let word = i[1].trim().chars().rev().collect::<String>();
            if word.len() != 32 {
                return Err(parse_error(ts!(
                    "error.word_size",
                    line = line,
                    got = word.len()
                )));
            }
            // Use a 64-bit temporarily before casting down later below, to avoid parsing errors on some cases
            let word: i64 = match i64::from_str_radix(&word, 2) {
                Ok(value) => value,
                Err(e) => {
                    return Err(parse_error(ts!(
                        "error.word_parse",
                        line = instruction,
                        word = word,
                        error = e
                    )));
                }
            };

            // The cast to i32 is safe because we ensured before there is only 32 bits of data in the word
            words[addr] = word as i32;
        }

        let store = Store {
            words,
            comments: Arc::new(comments),
        };
        store._check();

        Ok(store)
    }

    /// Set the source comment of the word at `index` (`None` clears it).
    ///
    /// This is how the TUI's inline comment editor writes an edit. The
    /// comments live behind an `Arc` shared with in-flight UI snapshots,
    /// so `Arc::make_mut` clones the array when a snapshot still holds a
    /// reference; the next snapshot then picks up the new text.
    pub fn set_comment(&mut self, index: usize, comment: Option<String>) {
        Arc::make_mut(&mut self.comments)[index] = comment;
    }

    /// Extract the opcode and data from the word at the given address
    ///
    /// Panics if the address is outside the store.
    pub fn decode_instruction(&self, address: usize) -> (Opcode, i32) {
        Self::decode_word(self.words[address])
    }

    /// Decode a raw 32-bit word into its opcode and operand.
    ///
    /// This is the same decoding the machine performs at fetch time, exposed
    /// as an associated function so callers with only a raw word (e.g. the
    /// TUI, which works from a state snapshot) can decode without a `Store`.
    ///
    /// # Word layout
    /// ```text
    /// word: 0b00000000000000000100000000011000
    ///                         ===        =====
    ///        Operation code ---'           |
    ///                  Data ---------------'
    /// ```
    pub fn decode_word(word: i32) -> (Opcode, i32) {
        // Step 1: extract instruction data (5 bits)
        let data = SSEM_DATA_MASK & word;

        // Step 2 & 3: shift right by 13, then mask to 3 bits for the opcode
        let opcode = (SSEM_OPCODE_MASK & (word >> SSEM_OPCODE_BIT_SHIFT)) as u8;

        (Opcode::from(opcode), data)
    }

    /// Rewrite the opcode field of a raw word, keeping every other bit.
    ///
    /// The inverse of `decode_word` for the opcode half: bits 13-15 are
    /// replaced, the data field (and any bits outside the two fields, which
    /// the machine ignores but a `NUM` word may carry) are preserved.
    pub fn with_opcode(word: i32, opcode: Opcode) -> i32 {
        (word & !(SSEM_OPCODE_MASK << SSEM_OPCODE_BIT_SHIFT))
            | ((opcode as i32) << SSEM_OPCODE_BIT_SHIFT)
    }

    /// Rewrite the data field of a raw word, keeping every other bit.
    ///
    /// The data is masked to 5 bits, like `decode_word` reads it.
    pub fn with_data(word: i32, data: i32) -> i32 {
        (word & !SSEM_DATA_MASK) | (data & SSEM_DATA_MASK)
    }

    /// Parse a word from human-readable input.
    ///
    /// Accepts either a decimal number (`42`, `-3`) or an instruction
    /// (`LDN 24`, `STO 26`, `CMP`). The `NUM` prefix stores the raw operand
    /// value, matching the assembler syntax. Opcode mnemonics are
    /// case-insensitive.
    ///
    /// # Examples
    /// ```text
    /// "42"      → 42
    /// "-3"      → -3
    /// "LDN 24"  → (2 << 13) | 24
    /// "CMP"     → (6 << 13) | 0
    /// "NUM 100" → 100
    /// ```
    pub fn parse_word(input: &str) -> Result<i32, String> {
        let input = input.trim();
        if input.is_empty() {
            return Err(ts!("error.empty_input"));
        }

        // Try a plain decimal number first
        if let Ok(n) = input.parse::<i32>() {
            return Ok(n);
        }

        // Try an instruction: "OPCODE [operand]"
        let tokens: Vec<&str> = input.split_ascii_whitespace().collect();
        match tokens.len() {
            1 | 2 => {
                let mnemonic = tokens[0].to_uppercase();
                let opcode = Opcode::from_str(&mnemonic)
                    .map_err(|_| ts!("error.unknown_opcode", mnemonic = mnemonic.as_str()))?;
                let operand: i32 = if tokens.len() == 2 {
                    tokens[1]
                        .parse()
                        .map_err(|_| ts!("error.invalid_operand", operand = tokens[1]))?
                } else {
                    0
                };

                match opcode {
                    Opcode::NUM => Ok(operand),
                    opcode => {
                        if !(0..=SSEM_DATA_MASK).contains(&operand) {
                            return Err(ts!("error.operand_range", max = SSEM_DATA_MASK));
                        }
                        Ok((opcode as i32) << SSEM_OPCODE_BIT_SHIFT | operand)
                    }
                }
            }
            _ => Err(ts!("error.expected_word")),
        }
    }

    /// Serialize the store as an assembly file's content, one line per word:
    /// ```
    /// 00 JMP 00  ; comment
    /// 01 LDN 24  ; A = -a
    /// 20 NUM -3  ; JRP offset
    /// ```
    /// A word is written as an instruction (`OPCODE operand`) only when no
    /// bit outside the opcode and data fields is set; any other bit pattern
    /// (a data word) is written as `NUM <value>`, which the assembler stores
    /// verbatim. Each word's source comment, when it has one, is appended
    /// after `;`, exactly as `from_asm_file` would parse it back.
    pub fn to_asm_string(&self) -> String {
        let mut out = String::new();
        for (index, &word) in self.words.iter().enumerate() {
            let (opcode, data) = Self::decode_word(word);
            // Re-encode the decoded fields: equal to the original word only
            // when every bit outside the opcode/data fields is clear
            let instruction = (opcode as i32) << SSEM_OPCODE_BIT_SHIFT | data;
            let body = if word == instruction {
                format!("{index:02} {opcode} {data:02}")
            } else {
                format!("{index:02} NUM {word}")
            };
            match &self.comments[index] {
                Some(comment) => out.push_str(&format!("{body};{comment}\n")),
                None => out.push_str(&format!("{body}\n")),
            }
        }
        out
    }

    /// Serialize the store as a snapshot file's content, one line per word:
    /// ```
    /// 0000: 10000000000000000000000000000000 ; comment
    /// 0001: 01010000000000100000000000000000
    /// ```
    /// Each word is written as its 32 bits in SSEM order (least significant
    /// bit first), with the source comment appended after `;` when present,
    /// exactly as `from_snp_file` would parse it back.
    pub fn to_snp_string(&self) -> String {
        let mut out = String::new();
        for (index, &word) in self.words.iter().enumerate() {
            let bits = format!("{:032b}", word.reverse_bits());
            let body = format!("{index:04}: {bits}");
            match &self.comments[index] {
                Some(comment) => out.push_str(&format!("{body};{comment}\n")),
                None => out.push_str(&format!("{body}\n")),
            }
        }
        out
    }

    /// Write the store to `path`, in the format given by the file extension
    /// (`.asm` or `.snp`), with each word's source comment if it has one.
    ///
    /// Returns an `InvalidInput` error when the extension is missing or
    /// unknown, and the underlying I/O error when the write fails.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let content = FileFormat::from_path(path)?.dump(self);
        std::fs::write(path, content)
    }

    fn _check(&self) {
        // Sanity checks
        if SSEM_DATA_MASK as usize >= SSEM_STORE_WORDS {
            panic!(
                "Invalid configuration: addressing size ({}) is bigger than store size ({})",
                SSEM_DATA_MASK + 1,
                SSEM_STORE_WORDS
            )
        }
        // INDEX_MASK only wraps correctly if the store size is a power of two
        if SSEM_STORE_WORDS.count_ones() != 1 {
            panic!("Invalid configuration: store size ({SSEM_STORE_WORDS}) must be a power of two")
        }
    }
}

impl Index<i32> for Store {
    type Output = i32;

    fn index(&self, index: i32) -> &Self::Output {
        let address = match usize::try_from(index) {
            Ok(value) => value,
            Err(_) => panic!("Unable to convert integer {} to usize", index),
        };
        let word: &i32 = match self.words.get(address) {
            Some(value) => value,
            None => panic!("Out of bound read at address {}", address),
        };
        word
    }
}

impl From<Vec<String>> for Store {
    fn from(value: Vec<String>) -> Self {
        if value.len() != SSEM_STORE_WORDS {
            panic!(
                "Invalid store size. Expected {SSEM_STORE_WORDS}, got {}",
                value.len()
            );
        }

        let mut words: [i32; SSEM_STORE_WORDS] = [0; SSEM_STORE_WORDS];
        for (index, w) in value.iter().enumerate() {
            if w.len() != 32 {
                panic!("Invalid word size '{}'", w);
            }
            // Reverse bit order to match modern representation
            let w = w.chars().rev().collect::<String>();
            // Use a 64-bit temporarily before casting down later below, to avoid parsing errors on some cases
            let parsed_word: i64 = match i64::from_str_radix(&w, 2) {
                Ok(value) => value,
                Err(e) => panic!(
                    "Unable to parse the word '{}' for address {}: {}",
                    w, index, e
                ),
            };
            words[index] = parsed_word as i32;
        }

        let store = Store {
            words,
            comments: Arc::new(std::array::from_fn(|_| None)),
        };
        store._check();
        store
    }
}

impl PartialEq for Store {
    fn eq(&self, other: &Self) -> bool {
        self.words == other.words
    }
}

/// A word rendered as its 32 bits in SSEM order (least significant bit first),
/// with '.' for a cleared bit and '#' for a set bit.
pub(crate) struct Bits(pub i32);

impl fmt::Display for Bits {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        const GLYPHS: [char; 2] = ['.', '#'];
        for i in 0..32 {
            f.write_char(GLYPHS[((self.0 >> i) & 1) as usize])?;
        }
        Ok(())
    }
}

impl fmt::Display for Store {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for (index, word) in self.words.iter().enumerate() {
            // Interpret the word as an instruction, the same way the machine does at fetch time
            let (opcode, data) = self.decode_instruction(index);
            // The source comment of the word's line, when the input file had
            // one, with its original indentation after the ';'
            let comment = self.comments[index]
                .as_deref()
                .map(|c| format!("  ;{c}"))
                .unwrap_or_default();
            writeln!(
                f,
                " {} {:02}:{:>12}  {} {:02}{comment}",
                Bits(*word),
                index,
                word,
                opcode,
                data
            )
            .ok();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use crate::ssem::opcode::Opcode;

    use super::{Bits, Store};

    /// A temporary file removed when dropped, even if the test panics
    struct TempFile(PathBuf);

    impl TempFile {
        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    /// Write the given content to a uniquely named temporary file
    fn temp_file(name: &str, content: &str) -> TempFile {
        let path = std::env::temp_dir().join(format!("ssem_store_tests_{name}"));
        let _ = std::fs::remove_file(&path); // Remove any leftover from a previous run
        std::fs::write(&path, content).expect("Unable to write the temporary test file");
        TempFile(path)
    }

    /// Encode an instruction the same way `from_asm_file` does
    fn instr(opcode: Opcode, data: i32) -> i32 {
        (opcode as i32) << 13 | data
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
        let store = Store::from(vec![
            "10000111111110001111111111111111".into(), // 00
            "01000111111111001111111111111111".into(), // 01
            "11000111111110101111111111111111".into(), // 02
            "00100111111111101111111111111111".into(), // 03
            "10100111111110011111111111111111".into(), // 04
            "01100111111111011111111111111111".into(), // 05
            "11100111111110111111111111111111".into(), // 06
            "00010111111111111111111111111111".into(), // 07
            "00000000000000000000000000000000".into(), // 08
            "00000000000000000000000000000000".into(), // 09
            "00000000000000000000000000000000".into(), // 10
            "00000000000000000000000000000000".into(), // 11
            "00000000000000000000000000000000".into(), // 12
            "00000000000000000000000000000000".into(), // 13
            "00000000000000000000000000000000".into(), // 14
            "00000000000000000000000000000000".into(), // 15
            "00000000000000000000000000000000".into(), // 16
            "00000000000000000000000000000000".into(), // 17
            "00000000000000000000000000000000".into(), // 18
            "00000000000000000000000000000000".into(), // 19
            "00000000000000000000000000000000".into(), // 20
            "00000000000000000000000000000000".into(), // 21
            "00000000000000000000000000000000".into(), // 22
            "00000000000000000000000000000000".into(), // 23
            "00000000000000000000000000000000".into(), // 24
            "00000000000000000000000000000000".into(), // 25
            "00000000000000000000000000000000".into(), // 26
            "00000000000000000000000000000000".into(), // 27
            "00000000000000000000000000000000".into(), // 28
            "00000000000000000000000000000000".into(), // 29
            "00000000000000000000000000000000".into(), // 30
            "00000000000000000000000000000000".into(), // 31
        ]);

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
        let s1 = Store::from(vec![
            "00000000000000000000000000000000".into(), // 00
            "10000000000000000000000000000000".into(), // 01
            "01000000000000000000000000000000".into(), // 02
            "11000000000000000000000000000000".into(), // 03
            "00100000000000000000000000000000".into(), // 04
            "10100000000000000000000000000000".into(), // 05
            "01100000000000000000000000000000".into(), // 06
            "11100000000000000000000000000000".into(), // 07
            "00010000000000000000000000000000".into(), // 08
            "10010000000000000000000000000000".into(), // 09
            "01010000000000000000000000000000".into(), // 10
            "11010000000000000000000000000000".into(), // 11
            "00110000000000000000000000000000".into(), // 12
            "10110000000000000000000000000000".into(), // 13
            "01110000000000000000000000000000".into(), // 14
            "11110000000000000000000000000000".into(), // 15
            "00001000000000000000000000000000".into(), // 16
            "10001000000000000000000000000000".into(), // 17
            "01001000000000000000000000000000".into(), // 18
            "11001000000000000000000000000000".into(), // 19
            "00101000000000000000000000000000".into(), // 20
            "10101000000000000000000000000000".into(), // 21
            "01101000000000000000000000000000".into(), // 22
            "11101000000000000000000000000000".into(), // 23
            "00011000000000000000000000000000".into(), // 24
            "10011000000000000000000000000000".into(), // 25
            "01011000000000000000000000000000".into(), // 26
            "11011000000000000000000000000000".into(), // 27
            "00111000000000000000000000000000".into(), // 28
            "10111000000000000000000000000000".into(), // 29
            "01111000000000000000000000000000".into(), // 30
            "11111000000000000000000000000000".into(), // 31
        ]);
        let s2 = Store::from(vec![
            "00000000000000000000000000000000".into(), // 00
            "10000000000000000000000000000000".into(), // 01
            "01000000000000000000000000000000".into(), // 02
            "11000000000000000000000000000000".into(), // 03
            "00100000000000000000000000000000".into(), // 04
            "10100000000000000000000000000000".into(), // 05
            "01100000000000000000000000000000".into(), // 06
            "11100000000000000000000000000000".into(), // 07
            "00010000000000000000000000000000".into(), // 08
            "10010000000000000000000000000000".into(), // 09
            "01010000000000000000000000000000".into(), // 10
            "11010000000000000000000000000000".into(), // 11
            "00110000000000000000000000000000".into(), // 12
            "10110000000000000000000000000000".into(), // 13
            "01110000000000000000000000000000".into(), // 14
            "11110000000000000000000000000000".into(), // 15
            "00001000000000000000000000000000".into(), // 16
            "10001000000000000000000000000000".into(), // 17
            "01001000000000000000000000000000".into(), // 18
            "11001000000000000000000000000000".into(), // 19
            "00101000000000000000000000000000".into(), // 20
            "10101000000000000000000000000000".into(), // 21
            "01101000000000000000000000000000".into(), // 22
            "11101000000000000000000000000000".into(), // 23
            "00011000000000000000000000000000".into(), // 24
            "10011000000000000000000000000000".into(), // 25
            "01011000000000000000000000000000".into(), // 26
            "11011000000000000000000000000000".into(), // 27
            "00111000000000000000000000000000".into(), // 28
            "10111000000000000000000000000000".into(), // 29
            "01111000000000000000000000000000".into(), // 30
            "11111000000000000000000000000000".into(), // 31
        ]);
        assert!(s1 == s2);

        // One word difference
        let s1 = Store::from(vec![
            "00000000000000000000000000000000".into(), // 00
            "10000000000000000000000000000000".into(), // 01
            "01000000000000000000000000000000".into(), // 02
            "11000000000000000000000000000000".into(), // 03
            "00100000000000000000000000000000".into(), // 04
            "10100000000000000000000000000000".into(), // 05
            "01100000000000000000000000000000".into(), // 06
            "11100000000000000000000000000000".into(), // 07
            "00010000000000000000000000000000".into(), // 08
            "10010000000000000000000000000000".into(), // 09
            "01010000000000000000000000000000".into(), // 10
            "11010000000000000000000000000000".into(), // 11
            "00110000000000000000000000000000".into(), // 12
            "10110000000000000000000000000000".into(), // 13
            "01110000000000000000000000000000".into(), // 14
            "11110000000000000000000000000000".into(), // 15
            "00001000000000000000000000000000".into(), // 16
            "10001000000000000000000000000000".into(), // 17
            "01001000000000000000000000000000".into(), // 18
            "11001000000000000000000000000000".into(), // 19
            "00101000000000000000000000000000".into(), // 20
            "10101000000000000000000000000000".into(), // 21
            "01101000000000000000000000000000".into(), // 22
            "11101000000000000000000000000000".into(), // 23
            "00011000000000000000000000000000".into(), // 24
            "10011000000000000000000000000000".into(), // 25
            "01011000000000000000000000000000".into(), // 26
            "11011000000000000000000000000000".into(), // 27
            "00111000000000000000000000000000".into(), // 28
            "10111000000000000000000000000000".into(), // 29
            "01111000000000000000000000000000".into(), // 30
            "11111000000000000000000000000000".into(), // 31
        ]);
        let s2 = Store::from(vec![
            "00000000000000000000000000000000".into(), // 00
            "10000000000000000000000000000000".into(), // 01
            "01000000000000000000000000000000".into(), // 02
            "11000000000000000000000000000000".into(), // 03
            "00100000000000000000000000000000".into(), // 04
            "10100000000000000000000000000000".into(), // 05
            "01100000000000000000000000000000".into(), // 06
            "11100000000000000000000000000000".into(), // 07
            "00010000000000000000000000000000".into(), // 08
            "10010000000000000000000000000000".into(), // 09
            "01010000000000000000000000000000".into(), // 10
            "11010000000000000000000000000000".into(), // 11
            "00110000000000000000000000000000".into(), // 12
            "10110000000000000000000000000000".into(), // 13
            "01110000000000000000000000000000".into(), // 14
            "11110000000000000000000000000000".into(), // 15
            "00001000000000000000000000000000".into(), // 16
            "10001000000000000000000000000000".into(), // 17
            "01001000000000000000000000000000".into(), // 18
            "11001000000000000000000000000000".into(), // 19
            "00101000000000000000000000000000".into(), // 20
            "10101000000000000000000000000000".into(), // 21
            "01101000000000000000000000000000".into(), // 22
            "11101000000000000000000000000000".into(), // 23
            "00011000000000000000000000000000".into(), // 24
            "10011000000000000000000000000000".into(), // 25
            "01011000000000000000000000000000".into(), // 26
            "11011000000000000000000000000000".into(), // 27
            "00111000000000000000000000000000".into(), // 28
            "10111000000000000000000000000000".into(), // 29
            "01111000000000000000000000000000".into(), // 30
            "11111000000000000000000000000001".into(), // 31 difference
        ]);
        assert!(s1 != s2);
    }

    #[test]
    fn index_i32() {
        let sample_store: Vec<String> = vec![
            "00000000000000000000000000000000".into(), // 00
            "10000000000000000000000000000000".into(), // 01
            "01000000000000000000000000000000".into(), // 02
            "11000000000000000000000000000000".into(), // 03
            "00000000000000000000000000000000".into(), // 04
            "00000000000000000000000000000000".into(), // 05
            "00000000000000000000000000000000".into(), // 06
            "00000000000000000000000000000000".into(), // 07
            "00000000000000000000000000000000".into(), // 08
            "00000000000000000000000000000000".into(), // 09
            "00000000000000000000000000000000".into(), // 10
            "00000000000000000000000000000000".into(), // 11
            "00000000000000000000000000000000".into(), // 12
            "00000000000000000000000000000000".into(), // 13
            "00000000000000000000000000000000".into(), // 14
            "00000000000000000000000000000000".into(), // 15
            "00000000000000000000000000000000".into(), // 16
            "00000000000000000000000000000000".into(), // 17
            "00000000000000000000000000000000".into(), // 18
            "00000000000000000000000000000000".into(), // 19
            "00000000000000000000000000000000".into(), // 20
            "00000000000000000000000000000000".into(), // 21
            "00000000000000000000000000000000".into(), // 22
            "00000000000000000000000000000000".into(), // 23
            "00000000000000000000000000000000".into(), // 24
            "00000000000000000000000000000000".into(), // 25
            "00000000000000000000000000000000".into(), // 26
            "00000000000000000000000000000000".into(), // 27
            "00000000000000000000000000000000".into(), // 28
            "00000000000000000000000000000000".into(), // 29
            "00000000000000000000000000000000".into(), // 30
            "00000000000000000000000000000000".into(), // 31
        ];
        let store = Store::from(sample_store);

        assert_eq!(1, store[1]);
        assert_eq!(2, store[2]);
        assert_eq!(3, store[3]);
    }

    #[test]
    #[should_panic]
    fn index_i32_error() {
        let sample_store: Vec<String> = vec![
            "00000000000000000000000000000000".into(), // 00
            "10000000000000000000000000000000".into(), // 01
            "01000000000000000000000000000000".into(), // 02
            "11000000000000000000000000000000".into(), // 03
            "00000000000000000000000000000000".into(), // 04
            "00000000000000000000000000000000".into(), // 05
            "00000000000000000000000000000000".into(), // 06
            "00000000000000000000000000000000".into(), // 07
            "00000000000000000000000000000000".into(), // 08
            "00000000000000000000000000000000".into(), // 09
            "00000000000000000000000000000000".into(), // 10
            "00000000000000000000000000000000".into(), // 11
            "00000000000000000000000000000000".into(), // 12
            "00000000000000000000000000000000".into(), // 13
            "00000000000000000000000000000000".into(), // 14
            "00000000000000000000000000000000".into(), // 15
            "00000000000000000000000000000000".into(), // 16
            "00000000000000000000000000000000".into(), // 17
            "00000000000000000000000000000000".into(), // 18
            "00000000000000000000000000000000".into(), // 19
            "00000000000000000000000000000000".into(), // 20
            "00000000000000000000000000000000".into(), // 21
            "00000000000000000000000000000000".into(), // 22
            "00000000000000000000000000000000".into(), // 23
            "00000000000000000000000000000000".into(), // 24
            "00000000000000000000000000000000".into(), // 25
            "00000000000000000000000000000000".into(), // 26
            "00000000000000000000000000000000".into(), // 27
            "00000000000000000000000000000000".into(), // 28
            "00000000000000000000000000000000".into(), // 29
            "00000000000000000000000000000000".into(), // 30
            "00000000000000000000000000000000".into(), // 31
        ];
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
}
