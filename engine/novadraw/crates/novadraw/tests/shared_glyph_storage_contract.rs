//! Independent consumers of immutable glyph storage.
use novadraw::{
    text::TextError,
    text::shaping::{GlyphBuffer, GlyphRange, PositionedGlyph},
};

#[test]
fn checked_ranges_preserve_order_and_retain_the_whole_backing() {
    let buffer = GlyphBuffer::new(
        (0..8)
            .map(|id| PositionedGlyph {
                id,
                x: id as f32,
                y: 11.0,
            })
            .collect(),
    )
    .unwrap();
    let middle = buffer.range(2..6).unwrap();
    let overlap = middle.slice(1..3).unwrap();
    assert_eq!(overlap.iter().map(|g| g.id).collect::<Vec<_>>(), [3, 4]);
    assert_eq!(overlap.glyph_indices(), 3..5);
    let owned = GlyphRange::new(overlap.to_vec()).unwrap();
    assert_eq!(
        owned, overlap,
        "glyph equality compares visible values, not backing or indices"
    );
    assert_eq!(
        middle.buffer().as_slice().as_ptr(),
        overlap.buffer().as_slice().as_ptr()
    );
    assert_eq!(overlap.buffer().as_slice().len(), 8);
    assert!(middle.slice(std::ops::Range { start: 3, end: 2 }).is_err());
    assert!(middle.slice(0..5).is_err());
    assert!(buffer.range(0..9).is_err());
    let empty = buffer.range(8..8).unwrap();
    assert!(empty.is_empty());
    drop(buffer);
    drop(middle);
    assert_eq!(overlap[0].id, 3);
    assert_eq!(empty.buffer().as_slice().len(), 8);
}

#[test]
fn owned_entry_rejects_non_finite_glyph_coordinates() {
    for (x, y) in [(f32::NAN, 0.0), (0.0, f32::INFINITY)] {
        assert_eq!(
            GlyphRange::new(vec![PositionedGlyph { id: 0, x, y }]),
            Err(TextError::InvalidGlyphRun),
        );
    }
    assert!(GlyphRange::new(vec![]).unwrap().is_empty());
}
