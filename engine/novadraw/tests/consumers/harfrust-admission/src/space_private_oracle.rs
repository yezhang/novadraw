//! Test-only ordinary reference for the non-public invisible buffer setting.
//! Injected only into isolated original/production library copies.
extern crate std;
use crate::{Direction, FontRef, ShapePlan, ShaperData, UnicodeBuffer};
use alloc::vec::Vec;

#[test]
fn ordinary_invisible_reference() {
    for space_present in [false, true] {
        let first: u16 = if space_present { 32 } else { 33 };
        let count = 123 - first;
        let mut cmap: Vec<u8> = [0u16, 1, 3, 1]
            .into_iter()
            .flat_map(u16::to_be_bytes)
            .collect();
        cmap.extend(12u32.to_be_bytes());
        cmap.extend(
            [6u16, 10 + 2 * count, 0, first, count]
                .into_iter()
                .flat_map(u16::to_be_bytes),
        );
        cmap.extend((first..123).flat_map(u16::to_be_bytes));
        let mut bytes = Vec::from(&b"\0\x01\0\0\0\x01\0\0\0\0\0\0cmap\0\0\0\0\0\0\0\x1c"[..]);
        bytes.extend((cmap.len() as u32).to_be_bytes());
        bytes.extend(cmap);
        let font = FontRef::new(&bytes).unwrap();
        let data = ShaperData::new(&font);
        let shaper = data.shaper(&font).build();
        for invisible in [None, Some(read_fonts::types::GlyphId::new(66))] {
            for direction in [Direction::LeftToRight, Direction::RightToLeft] {
                let plan = ShapePlan::new(&shaper, direction, None, None, &[]);
                for text in [
                    "\u{2007}",
                    "\u{2008}",
                    "\u{202f}",
                    "\u{1680}",
                    "\u{2000}\u{2001}\u{2011}",
                ] {
                    let mut buffer = UnicodeBuffer::new();
                    buffer.push_str(text);
                    buffer.set_direction(direction);
                    buffer.0.invisible = invisible;
                    let output = shaper.shape_with_plan(&plan, buffer, &[]);
                    std::println!(
                        "SPACE/{space_present}/{invisible:?}/{direction:?}/{text:?}/{:?}/{:?}",
                        output.glyph_infos(),
                        output.glyph_positions()
                    );
                }
            }
        }
    }
}
