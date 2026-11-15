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

/// Parse a store from an indexed text file — the skeleton shared by
/// `from_asm_file` and `from_snp_file`: open the file, split each line into
/// its instruction and `;` comment, skip comment-only lines, check that
/// indices are contiguous from 0 and within the store, record each word's
/// comment, and assemble the `Store`.
///
/// `parse_line` is the format-specific part: given the trimmed instruction
/// (comment removed) and the raw line, it returns the word's index and value.
fn parse_indexed_file(
    filename: &Path,
    parse_line: impl Fn(&str, &str) -> std::io::Result<(i32, i32)>,
) -> std::io::Result<Store> {
    let file = File::open(filename).map_err(|e| read_error(filename, e))?;
    let reader = BufReader::new(file);

    let mut words = [0_i32; SSEM_STORE_WORDS];
    let mut comments = std::array::from_fn(|_| None);

    let mut last_index: Option<i32> = None;

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

        let (index, word) = parse_line(instruction, &line)?;

        // Ensure lines are contiguous and without duplicates: each index
        // must be exactly one past the previous one. A repeated index
        // fails this too, including a second index-0 line.
        match last_index {
            Some(last) if index != last + 1 => {
                return Err(parse_error(ts!(
                    "error.expected_index",
                    line = line,
                    expected = last + 1
                )));
            }
            None if index > 1 => {
                return Err(parse_error(ts!(
                    "error.expected_index",
                    line = line,
                    expected = 1
                )));
            }
            _ => {}
        }
        last_index = Some(index);

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
        words[addr] = word;
    }

    let store = Store {
        words,
        comments: Arc::new(comments),
    };
    store._check();

    Ok(store)
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
        parse_indexed_file(filename, |instruction, _line| {
            // Extracting tokens "<index> <opcode> <operand>"; anything
            // beyond the operand is rejected, not silently ignored
            let i: Vec<&str> = instruction.split_ascii_whitespace().collect();
            if i.len() < 2 || i.len() > 3 {
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

            let opcode = Opcode::from_str(opcode).map_err(|_| {
                parse_error(ts!(
                    "error.opcode_not_valid",
                    line = instruction,
                    opcode = opcode
                ))
            })?;

            let word = match opcode {
                Opcode::NUM => operand,
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
                    (opcode as i32) << SSEM_OPCODE_BIT_SHIFT | operand
                }
            };
            Ok((index, word))
        })
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
        parse_indexed_file(filename, |instruction, line| {
            // Extracting tokens "<index>: <binary_word>"
            let i: Vec<&str> = instruction.splitn(2, ':').collect();
            if i.len() != 2 {
                return Err(parse_error(ts!("error.invalid_syntax", line = instruction)));
            }

            let index: i32 = i[0]
                .parse()
                .map_err(|_| parse_error(ts!("error.read_number")))?;

            // Reverse the bit order: SSEM is least significant bit first
            let word = i[1].trim().chars().rev().collect::<String>();
            if word.len() != 32 {
                return Err(parse_error(ts!(
                    "error.word_size",
                    line = line,
                    got = word.len()
                )));
            }
            // Use a 64-bit temporarily before casting down below, to avoid parsing errors on some cases
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

            // The cast to i32 is safe because we ensured above there is only 32 bits of data in the word
            Ok((index, word as i32))
        })
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
        crate::atomic_write::write_atomic(path, &content)
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
