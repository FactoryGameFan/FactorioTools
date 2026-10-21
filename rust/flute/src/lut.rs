//! The lookup table, from `LookUpTable.cs` and `Csoln.cs` in FluteSharp.
//!
//! The reader is a byte-for-byte copy of the C# one, which was ported from Stefan Mücke's
//! Java `Flute.java`. It keeps that reader's quirks on purpose: one 32-byte line buffer
//! shared by both files and never cleared between lines, and the same mapping from
//! characters to numbers. Matching the C# exactly is the point, so the quirks stay.

use crate::FluteError;

/// `LookUpTable.numgrp`.
pub(crate) const NUMGRP: [usize; 10] = [0, 0, 0, 0, 6, 30, 180, 1260, 10080, 90720];

/// `Csoln`: one stored solution.
#[derive(Clone, Debug)]
pub(crate) struct Csoln {
    pub(crate) parent: u8,
    /// Add: 0..i, Sub: j..10; seg[i+1] = seg[j-1] = 0.
    pub(crate) seg: [u8; 11],
    /// row = rowcol / 16, col = rowcol % 16.
    pub(crate) rowcol: Vec<u8>,
    pub(crate) neighbor: Vec<u8>,
}

impl Csoln {
    /// `new Csoln(D)`. The sizes come from the table's maximum degree, not the group's.
    fn new(d: usize) -> Self {
        Csoln {
            parent: 0,
            seg: [0; 11],
            rowcol: vec![0; d - 2],
            neighbor: vec![0; 2 * d - 2],
        }
    }
}

/// `LookUpTable`.
#[derive(Clone, Debug)]
pub struct LookUpTable {
    /// `D`: the maximum degree the table supports.
    pub(crate) d: usize,
    /// `MPOWV`: the maximum number of POWVs per group.
    pub(crate) mpowv: usize,
    /// `LUT[d, k]`, as an index into `solutions`. The C# shares one array between groups
    /// that the file marks as "same as group kk", so this shares an index.
    lut: Vec<Vec<Option<usize>>>,
    solutions: Vec<Vec<Csoln>>,
    /// `numsoln[d, k]`.
    numsoln: Vec<Vec<usize>>,
}

const POWV6: &[u8] = include_bytes!("../data/POWV6.dat");
const POST6: &[u8] = include_bytes!("../data/POST6.dat");

impl LookUpTable {
    /// `LookUpTable.Degree6`, built from the same two files FluteSharp's source generator
    /// embeds.
    pub fn degree6() -> Self {
        LookUpTable::new(6, POWV6, POST6).expect("the embedded degree 6 tables are valid")
    }

    /// `new LookUpTable(d, byte[] powv, byte[] post)`.
    pub fn new(d: usize, powv: &[u8], post: &[u8]) -> Result<Self, FluteError> {
        if !(4..=9).contains(&d) {
            return Err(FluteError::DegreeOutOfRange(d));
        }
        let (mgroup, mpowv) = match d {
            ..=7 => (5040 / 4, 15),
            8 => (40320 / 4, 33),
            _ => (362880 / 4, 79),
        };
        let mut table = LookUpTable {
            d,
            mpowv,
            lut: vec![vec![None; mgroup]; d + 1],
            solutions: Vec::new(),
            numsoln: vec![vec![0; mgroup]; d + 1],
        };
        let mut powv = ByteReader::new(powv);
        let mut post = ByteReader::new(post);
        table.read_lut(&mut powv, &mut post)?;
        Ok(table)
    }

    /// The solutions for group `k` of degree `d`: `LUT[d, k]`.
    pub(crate) fn group(&self, d: usize, k: usize) -> &[Csoln] {
        let index = self.lut[d][k].expect("every group of every degree is read");
        &self.solutions[index]
    }

    /// `numsoln[d, k]`.
    pub(crate) fn numsoln(&self, d: usize, k: usize) -> usize {
        self.numsoln[d][k]
    }

    /// `readLUT`.
    fn read_lut(&mut self, powv: &mut ByteReader, post: &mut ByteReader) -> Result<(), FluteError> {
        let charnum = charnum();
        // `char[] lineBuf = new char[32]`, shared by both files and never cleared.
        let mut line_buf = [0u8; 32];

        let mut d = 4;
        while d <= self.d {
            // d=%d\n in POWV. The C# assigns d here too, and the POST line below
            // overwrites it, so only the POST value is kept.
            read_line(powv, &mut line_buf);
            let linep = scan_string(&line_buf, 0, b"d=")?;
            let (_, next) = scan_number(&line_buf, linep)?;
            scan_eol(&line_buf, next)?;

            // d=%d\n in POST.
            read_line(post, &mut line_buf);
            let linep = scan_string(&line_buf, 0, b"d=")?;
            let (number, next) = scan_number(&line_buf, linep)?;
            d = number;
            scan_eol(&line_buf, next)?;

            for k in 0..NUMGRP[d] {
                let ns = charnum[(powv.read_byte() & 0xff) as usize] as usize;

                if ns == 0 {
                    // Same as some previous group. %d\n
                    read_line(powv, &mut line_buf);
                    let (kk, next) = scan_number(&line_buf, 0)?;
                    scan_eol(&line_buf, next)?;
                    self.numsoln[d][k] = self.numsoln[d][kk];
                    self.lut[d][k] = self.lut[d][kk];
                } else {
                    powv.read_byte(); // '\n'
                    self.numsoln[d][k] = ns;
                    let mut p = vec![Csoln::new(self.d); ns];
                    for solution in &mut p {
                        read_line(powv, &mut line_buf);
                        let mut linep = 0;
                        solution.parent = charnum[line_buf[linep] as usize];
                        linep += 1;
                        let mut j = 0;
                        loop {
                            let b = charnum[line_buf[linep] as usize];
                            linep += 1;
                            solution.seg[j] = b;
                            j += 1;
                            if b == 0 {
                                break;
                            }
                        }
                        j = 10;
                        loop {
                            let b = charnum[line_buf[linep] as usize];
                            linep += 1;
                            solution.seg[j] = b;
                            j = j.wrapping_sub(1);
                            if b == 0 {
                                break;
                            }
                        }
                        let nn = 2 * d - 2;
                        read_chars(post, &mut line_buf, d - 2);
                        // j runs d..nn in the C#, reading lineBuf from 0.
                        for (slot, &c) in solution.rowcol.iter_mut().zip(&line_buf[..nn - d]) {
                            *slot = charnum[c as usize];
                        }
                        read_chars(post, &mut line_buf, nn / 2 + 1); // last char \n
                        let mut linep = 0;
                        let mut j = 0;
                        while j < nn {
                            let c = line_buf[linep];
                            linep += 1;
                            solution.neighbor[j] = c / 16;
                            solution.neighbor[j + 1] = c % 16;
                            j += 2;
                        }
                    }
                    self.lut[d][k] = Some(self.solutions.len());
                    self.solutions.push(p);
                }
            }
            d += 1;
        }
        Ok(())
    }
}

/// The C# `charnum` table: digits to 0-9, `A` and up to 10 and up, everything else to 0.
fn charnum() -> [u8; 256] {
    let mut charnum = [0u8; 256];
    for (i, slot) in charnum.iter_mut().enumerate() {
        *slot = if (b'0' as usize..=b'9' as usize).contains(&i) {
            (i - b'0' as usize) as u8
        } else if i >= b'A' as usize {
            (i - b'A' as usize + 10) as u8
        } else {
            0
        };
    }
    charnum
}

/// `ByteReader`: returns each byte, then -1 forever once the data runs out.
struct ByteReader<'a> {
    data: &'a [u8],
    index: usize,
}

impl<'a> ByteReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        ByteReader { data, index: 0 }
    }

    fn read_byte(&mut self) -> i32 {
        match self.data.get(self.index) {
            Some(&b) => {
                self.index += 1;
                i32::from(b)
            }
            None => -1,
        }
    }
}

/// `readLine`: copies bytes up to and including the next `\n` into the start of `buf`.
/// The rest of `buf` keeps whatever an earlier line left there, as in the C#.
fn read_line(reader: &mut ByteReader, buf: &mut [u8; 32]) {
    let mut i = 0;
    loop {
        let c = reader.read_byte();
        if c == -1 {
            break;
        }
        if c == i32::from(b'\n') {
            buf[i] = b'\n';
            break;
        }
        buf[i] = (c & 0xff) as u8;
        i += 1;
    }
}

/// `readChars`.
fn read_chars(reader: &mut ByteReader, buf: &mut [u8; 32], count: usize) {
    for slot in buf.iter_mut().take(count) {
        *slot = (reader.read_byte() & 0xff) as u8;
    }
}

/// `scanNumber`: returns the number and the offset of the first byte after it.
fn scan_number(buf: &[u8; 32], mut offset: usize) -> Result<(usize, usize), FluteError> {
    let c = buf[offset];
    offset += 1;
    if !c.is_ascii_digit() {
        return Err(FluteError::InvalidData(format!(
            "Reading error. Expected digit but got '{c}'."
        )));
    }
    let mut number = usize::from(c - b'0');
    loop {
        let c = buf[offset];
        offset += 1;
        if !c.is_ascii_digit() {
            break;
        }
        number = number * 10 + usize::from(c - b'0');
    }
    Ok((number, offset - 1))
}

/// `scanString`.
fn scan_string(buf: &[u8; 32], offset: usize, expected: &[u8]) -> Result<usize, FluteError> {
    for (i, &e) in expected.iter().enumerate() {
        let c = buf[offset + i];
        if c != e {
            return Err(FluteError::InvalidData(format!(
                "Reading error. Expected '{}' but got '{c}'.",
                char::from(e)
            )));
        }
    }
    Ok(offset + expected.len())
}

/// `scanEOL`.
fn scan_eol(buf: &[u8; 32], offset: usize) -> Result<(), FluteError> {
    if buf[offset] != b'\n' {
        return Err(FluteError::InvalidData(format!(
            "Reading error. Expected EOL but got '{}'.",
            buf[offset]
        )));
    }
    Ok(())
}
