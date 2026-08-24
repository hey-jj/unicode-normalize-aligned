//! Per-character property lookup and the table-backed decomposition and
//! composition queries.
//!
//! Every query starts with one packed `u16` value from the two-level trie in
//! `tables.rs`. ASCII never touches the tables.

use crate::tables::{
    ASCII_COMPOSES_FIRST, CANON, COMPAT, COMPOSE, POOL, PROPS_INDEX, PROPS_LEAVES, PROPS_SHIFT,
    PROPS_VALUES,
};

const CCC_MASK: u16 = 0x00FF;
const COMBINING_MARK: u16 = 1 << 8;
const NFD_NO: u16 = 1 << 9;
const HAS_COMPAT: u16 = 1 << 10;
const NFC_QC_SHIFT: u16 = 11;
const NFC_QC_MASK: u16 = 0b11 << NFC_QC_SHIFT;
const COMPOSES_FIRST: u16 = 1 << 13;

const QC_NO: u16 = 1;
const QC_MAYBE: u16 = 2;

/// Quick-check answer for one character.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum QuickCheck {
    Yes,
    No,
    Maybe,
}

/// Packed properties of one character.
///
/// Bit layout, matching the generator:
/// bits 0-7 canonical combining class, bit 8 General_Category in {Mn, Mc, Me},
/// bit 9 NFD_QC No (the character has a canonical decomposition, Hangul
/// syllables included), bit 10 the full compatibility decomposition differs
/// from the canonical one, bits 11-12 NFC_QC (0 Yes, 1 No, 2 Maybe), bit 13 the
/// character is the first of a primary composite pair or an L jamo or LV
/// syllable. NFKD_QC No is bit 9 or bit 10. NFKC_QC is No when NFC_QC is No or
/// bit 10 is set, and equals NFC_QC otherwise. A character can be the second
/// of a primary composite pair only when NFC_QC is Maybe. The generator
/// asserts each of those equivalences against DerivedNormalizationProps.
#[derive(Clone, Copy)]
pub(crate) struct Props(u16);

impl Props {
    #[inline]
    pub(crate) fn of(c: char) -> Props {
        let cp = c as u32;
        if cp < 0x80 {
            // ASCII has no property except membership in a composite pair
            // ('A' + U+0300 gives U+00C0). The generator asserts that.
            let word = ASCII_COMPOSES_FIRST[usize::from(cp >= 64)];
            let bit = (word >> (cp & 63)) & 1;
            return Props((bit as u16) << 13);
        }
        let block = PROPS_INDEX
            .get((cp >> PROPS_SHIFT) as usize)
            .copied()
            .unwrap_or(0);
        let low = cp & ((1 << PROPS_SHIFT) - 1);
        let slot = ((u32::from(block)) << PROPS_SHIFT) | low;
        let leaf = PROPS_LEAVES.get(slot as usize).copied().unwrap_or(0);
        Props(PROPS_VALUES.get(usize::from(leaf)).copied().unwrap_or(0))
    }

    #[inline]
    pub(crate) fn ccc(self) -> u8 {
        (self.0 & CCC_MASK) as u8
    }

    #[inline]
    pub(crate) fn is_combining_mark(self) -> bool {
        self.0 & COMBINING_MARK != 0
    }

    #[inline]
    pub(crate) fn nfd_no(self) -> bool {
        self.0 & NFD_NO != 0
    }

    #[inline]
    pub(crate) fn has_compat(self) -> bool {
        self.0 & HAS_COMPAT != 0
    }

    #[inline]
    pub(crate) fn nfkd_no(self) -> bool {
        self.0 & (NFD_NO | HAS_COMPAT) != 0
    }

    #[inline]
    pub(crate) fn nfc_qc(self) -> QuickCheck {
        match (self.0 & NFC_QC_MASK) >> NFC_QC_SHIFT {
            QC_NO => QuickCheck::No,
            QC_MAYBE => QuickCheck::Maybe,
            _ => QuickCheck::Yes,
        }
    }

    #[inline]
    pub(crate) fn nfkc_qc(self) -> QuickCheck {
        if self.has_compat() {
            QuickCheck::No
        } else {
            self.nfc_qc()
        }
    }

    #[inline]
    pub(crate) fn composes_first(self) -> bool {
        self.0 & COMPOSES_FIRST != 0
    }

    #[inline]
    pub(crate) fn composes_second(self) -> bool {
        (self.0 & NFC_QC_MASK) >> NFC_QC_SHIFT == QC_MAYBE
    }
}

/// Full canonical decomposition from the tables. `None` when the character
/// has no canonical mapping. Hangul syllables are not in the tables.
#[inline]
pub(crate) fn canonical(c: char) -> Option<&'static [char]> {
    lookup(CANON, c)
}

/// Full compatibility decomposition for a character whose `has_compat` bit
/// is set, so its compatibility expansion differs from the canonical one and
/// sits in `COMPAT`. Every other character's compatibility decomposition is
/// its canonical one.
#[inline]
pub(crate) fn compat_distinct(c: char) -> Option<&'static [char]> {
    lookup(COMPAT, c)
}

fn lookup(table: &'static [(u32, u16, u8)], c: char) -> Option<&'static [char]> {
    let i = table.binary_search_by_key(&(c as u32), |e| e.0).ok()?;
    let &(_, offset, len) = table.get(i)?;
    let start = usize::from(offset);
    POOL.get(start..start + usize::from(len))
}

/// Primary composite for the pair, from the table. Hangul is handled by the
/// caller.
#[inline]
pub(crate) fn compose_pair(a: char, b: char) -> Option<char> {
    let key = ((a as u64) << 21) | (b as u64);
    let i = COMPOSE.binary_search_by_key(&key, |e| e >> 21).ok()?;
    let packed = COMPOSE.get(i)?;
    char::from_u32((packed & 0x1F_FFFF) as u32)
}

pub(crate) mod hangul {
    //! Arithmetic Hangul decomposition and composition (Unicode chapter 3.12).

    const S_BASE: u32 = 0xAC00;
    const L_BASE: u32 = 0x1100;
    const V_BASE: u32 = 0x1161;
    const T_BASE: u32 = 0x11A7;
    const L_COUNT: u32 = 19;
    const V_COUNT: u32 = 21;
    const T_COUNT: u32 = 28;
    const N_COUNT: u32 = V_COUNT * T_COUNT;
    const S_COUNT: u32 = L_COUNT * N_COUNT;

    #[inline]
    fn to_char(cp: u32) -> char {
        char::from_u32(cp).unwrap_or('\u{FFFD}')
    }

    /// Splits a syllable into L, V and an optional T. `None` outside the
    /// syllable block.
    #[inline]
    pub(crate) fn decompose(c: char) -> Option<(char, char, Option<char>)> {
        let s = (c as u32).wrapping_sub(S_BASE);
        if s >= S_COUNT {
            return None;
        }
        let l = to_char(L_BASE + s / N_COUNT);
        let v = to_char(V_BASE + (s % N_COUNT) / T_COUNT);
        let t = s % T_COUNT;
        let t = if t == 0 {
            None
        } else {
            Some(to_char(T_BASE + t))
        };
        Some((l, v, t))
    }

    /// L + V gives LV. LV + T gives LVT. Nothing else composes.
    #[inline]
    pub(crate) fn compose(a: char, b: char) -> Option<char> {
        let (a, b) = (a as u32, b as u32);
        let l = a.wrapping_sub(L_BASE);
        let v = b.wrapping_sub(V_BASE);
        if l < L_COUNT && v < V_COUNT {
            return Some(to_char(S_BASE + l * N_COUNT + v * T_COUNT));
        }
        let s = a.wrapping_sub(S_BASE);
        let t = b.wrapping_sub(T_BASE);
        if s < S_COUNT && s % T_COUNT == 0 && t > 0 && t < T_COUNT {
            return Some(to_char(a + t));
        }
        None
    }
}
