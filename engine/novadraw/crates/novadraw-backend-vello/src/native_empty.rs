//! Backend-local trial: only structurally empty, monochrome Glyf records.
//! This is not a device coverage certificate or a Core recording capability.
use skrifa::{FontRef, MetadataProvider, outline::OutlineGlyphFormat, raw::TableProvider};

const BITS_PER_WORD: usize = u64::BITS as usize;
pub(super) const MAX_CACHED_FACES: usize = 16;

#[derive(Default)]
pub(super) struct EmptyGlyphs {
    words: Vec<u64>,
}

impl EmptyGlyphs {
    pub(super) fn new(bytes: &[u8], index: u32) -> Self {
        Self::build(bytes, index).unwrap_or_default()
    }

    fn build(bytes: &[u8], index: u32) -> Option<Self> {
        let face = FontRef::from_index(bytes, index).ok()?;
        if (face.colr().is_ok() && face.cpal().is_ok()) || !face.bitmap_strikes().is_empty() {
            return None;
        }
        let outlines = face.outline_glyphs();
        // VARC takes precedence over Glyf even if the font also contains glyf tables.
        if outlines.format() != Some(OutlineGlyphFormat::Glyf) {
            return None;
        }
        let count = usize::from(face.maxp().ok()?.num_glyphs());
        let head = face.head().ok()?;
        let loca = face.loca(Some(head.index_to_loc_format() == 1)).ok()?;
        let glyf = face.glyf().ok()?;
        let mut words = Vec::new();
        words
            .try_reserve_exact(count.div_ceil(BITS_PER_WORD))
            .ok()?;
        words.resize(count.div_ceil(BITS_PER_WORD), 0);
        let mut any = false;
        for glyph in 0..count {
            let id = skrifa::GlyphId::new(glyph as u32);
            if outlines.get(id).is_some() && matches!(loca.get_glyf(id, &glyf), Ok(None)) {
                words[glyph / BITS_PER_WORD] |= 1 << (glyph % BITS_PER_WORD);
                any = true;
            }
        }
        any.then_some(Self { words })
    }

    pub(super) fn contains(&self, glyph: u32) -> bool {
        let glyph = glyph as usize;
        self.words
            .get(glyph / BITS_PER_WORD)
            .is_some_and(|word| word & (1 << (glyph % BITS_PER_WORD)) != 0)
    }

    pub(super) fn is_empty(&self) -> bool {
        self.words.is_empty()
    }

    #[cfg(test)]
    pub(super) fn storage_bytes(&self) -> usize {
        self.words.capacity() * std::mem::size_of::<u64>()
    }
}
