//! Isolated Parley feasibility evidence. Does not exercise Core publication or GPU.
use parley::{
    Alignment, AlignmentOptions, FontContext, FontFamily, Layout, LayoutContext,
    PositionedLayoutItem, StyleProperty,
    editing::{Cursor, Selection},
    layout::Affinity,
};
use serde_json::json;
use std::{
    alloc::{GlobalAlloc, Layout as AllocationLayout, System},
    hint::black_box,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Instant,
};

// Single-threaded instrumentation only; not an engine design.
struct Meter;
static ACTIVE: AtomicBool = AtomicBool::new(false);
static ALLOCS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);
unsafe impl GlobalAlloc for Meter {
    unsafe fn alloc(&self, layout: AllocationLayout) -> *mut u8 {
        if ACTIVE.load(Ordering::Relaxed) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
            BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        }
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: AllocationLayout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: AllocationLayout, size: usize) -> *mut u8 {
        if ACTIVE.load(Ordering::Relaxed) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
            BYTES.fetch_add(size as u64, Ordering::Relaxed);
        }
        unsafe { System.realloc(ptr, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: Meter = Meter;

fn timed<T>(f: impl FnOnce() -> T) -> (T, serde_json::Value) {
    ALLOCS.store(0, Ordering::Relaxed);
    BYTES.store(0, Ordering::Relaxed);
    ACTIVE.store(true, Ordering::Relaxed);
    let start = Instant::now();
    let result = black_box(f());
    let ns = start.elapsed().as_nanos() as u64;
    ACTIVE.store(false, Ordering::Relaxed);
    let allocs = ALLOCS.load(Ordering::Relaxed);
    let bytes = BYTES.load(Ordering::Relaxed);
    (
        result,
        json!({"ns": ns, "allocations": allocs, "requested_bytes": bytes}),
    )
}

fn shape(
    fc: &mut FontContext,
    lc: &mut LayoutContext<usize>,
    text: &str,
    split: usize,
) -> Layout<usize> {
    let families =
        FontFamily::parse_list("Inter Variable, Noto Sans SC, Noto Sans Arabic, JetBrains Mono")
            .collect::<Vec<_>>();
    let mut builder = lc.ranged_builder(fc, text, 1.0, true);
    builder.push_default(families.as_slice());
    builder.push_default(StyleProperty::FontSize(12.0));
    builder.push_default(StyleProperty::Brush(0));
    // Stable multi-style runs, with endpoints on code point boundaries.
    builder.push(StyleProperty::FontSize(15.0), split..text.len());
    builder.push(StyleProperty::Brush(1), split..text.len());
    builder.build(text)
}

#[derive(Debug, PartialEq)]
struct Output {
    dimensions: [f32; 3],
    lines: Vec<(std::ops::Range<usize>, parley::layout::LineMetrics)>,
    glyphs: Vec<(u32, f32, f32, f32)>,
    runs: Vec<(std::ops::Range<usize>, u64, u32, f32, bool, usize, Vec<i16>)>,
    clusters: Vec<(std::ops::Range<usize>, f32)>,
}

fn export(layout: &Layout<usize>) -> Output {
    let mut output = Output {
        dimensions: [layout.width(), layout.full_width(), layout.height()],
        lines: Vec::new(),
        glyphs: Vec::new(),
        runs: Vec::new(),
        clusters: Vec::new(),
    };
    for line in layout.lines() {
        output.lines.push((line.text_range(), *line.metrics()));
        for item in line.items() {
            let PositionedLayoutItem::GlyphRun(gr) = item else {
                panic!("This experiment deliberately excludes inline boxes");
            };
            let run = gr.run();
            output.runs.push((
                run.text_range(),
                run.font().data.id(),
                run.font().index,
                run.font_size(),
                run.is_rtl(),
                gr.style().brush,
                run.normalized_coords().to_vec(),
            ));
            // Same Y convention as current Core export.
            let mut x = gr.offset();
            for glyph in gr.glyphs() {
                output.glyphs.push((
                    glyph.id,
                    x + glyph.x,
                    gr.baseline() - glyph.y,
                    glyph.advance,
                ));
                x += glyph.advance;
            }
        }
        for run in line.runs() {
            for cluster in run.visual_clusters() {
                output
                    .clusters
                    .push((cluster.text_range(), cluster.advance()));
            }
        }
    }
    output
}

fn interaction(layout: &Layout<usize>, text: &str, exhaustive: bool) -> Vec<String> {
    let boundaries = text
        .char_indices()
        .map(|(i, _)| i)
        .chain(std::iter::once(text.len()))
        .collect::<Vec<_>>();
    let mut result = Vec::new();
    for (ordinal, offset) in boundaries.iter().enumerate() {
        if !exhaustive && ordinal % (boundaries.len() / 32).max(1) != 0 {
            continue;
        }
        for affinity in [Affinity::Upstream, Affinity::Downstream] {
            let c = Cursor::from_byte_index(layout, *offset, affinity);
            let g = c.geometry(layout, 0.0);
            let hit = Cursor::from_point(
                layout,
                ((g.x0 + g.x1) * 0.5) as f32,
                ((g.y0 + g.y1) * 0.5) as f32,
            );
            let bounds = [g.x0, g.y0, g.x1, g.y1];
            result.push(format!(
                "{c:?}|{bounds:?}|{hit:?}|{:?}|{:?}",
                c.previous_visual(layout),
                c.next_visual(layout)
            ));
        }
    }
    let start = Cursor::from_byte_index(layout, 0, Affinity::Downstream);
    let end = Cursor::from_byte_index(layout, text.len(), Affinity::Upstream);
    let quads = Selection::new(start, end)
        .geometry(layout)
        .into_iter()
        .map(|(g, line)| ([g.x0, g.y0, g.x1, g.y1], line))
        .collect::<Vec<_>>();
    result.push(format!("{quads:?}"));
    result
}

fn finish(layout: &mut Layout<usize>, width: Option<f32>, alignment: Alignment) {
    layout.break_all_lines(width);
    layout.align(width, alignment, AlignmentOptions::default());
}

fn main() {
    const SAMPLES: usize = 96;
    const WARMUP: usize = 8;
    let mut fc = FontContext::new();
    let mut lc = LayoutContext::new();
    let font_root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../assets/fonts");
    for name in [
        "InterVariable.ttf",
        "NotoSansSC-VF.ttf",
        "NotoSansArabic-VF.ttf",
        "JetBrainsMono-Regular.ttf",
    ] {
        let bytes = std::fs::read(font_root.join(name)).unwrap();
        fc.collection
            .register_fonts(parley::fontique::Blob::new(Arc::new(bytes)), None);
    }
    let workloads = [
        (
            "latin-large",
            "Text rendering office affinity ffi AV To tabs\tand words. ".repeat(950),
        ),
        (
            "mixed-large",
            "中文排版，换行与字体。 العربية لاختبار النص office ffi 123\n".repeat(750),
        ),
        (
            "rtl-large",
            "\u{200f}العربية لاختبار النص والأحرف المتصلة 123 office ".repeat(750),
        ),
        (
            "complex-small",
            "Aé 👩‍👩‍👧‍👦 ffi العربية 中文\txyz\nsecond line".to_string(),
        ),
    ];
    let widths = [
        Some(640.0),
        Some(220.0),
        Some(960.0),
        Some(640.0),
        Some(1.0),
        None,
    ];
    let aligns = [
        Alignment::Start,
        Alignment::Center,
        Alignment::End,
        Alignment::Left,
        Alignment::Right,
    ];
    println!(
        "{}",
        json!({"kind":"metadata","parley":"0.7.0","samples":SAMPLES,
        "warmup":WARMUP,"scope":"Parley only; full glyph/line/run/cluster export; no Core publication or GPU",
        "allocator":"System with allocation/reallocation requested bytes; not RSS or peak live bytes"})
    );
    for (name, text) in workloads {
        let split = text
            .char_indices()
            .nth(text.chars().count() / 2)
            .map_or(text.len(), |(offset, _)| offset);
        let (template, creation) = timed(|| shape(&mut fc, &mut lc, &text, split));
        // Retain a previously aligned layout as an extra isolation/state-reset case.
        let mut previous = template.clone();
        finish(&mut previous, widths[0], Alignment::Center);
        let previous_output = export(&previous);
        let mut comparisons = 0;
        for width in widths {
            for alignment in aligns {
                let mut fresh = shape(&mut fc, &mut lc, &text, split);
                finish(&mut fresh, width, alignment);
                let expected = export(&fresh);
                let expected_interaction = interaction(&fresh, &text, name == "complex-small");
                for source in [&template, &previous] {
                    let mut reused = source.clone();
                    finish(&mut reused, width, alignment);
                    assert_eq!(export(&reused), expected, "{name} {width:?} {alignment:?}");
                    assert_eq!(
                        interaction(&reused, &text, name == "complex-small"),
                        expected_interaction,
                        "interaction {name} {width:?} {alignment:?}"
                    );
                    comparisons += 1;
                }
            }
        }
        assert_eq!(export(&previous), previous_output, "old snapshot changed");
        assert!(template.is_empty(), "template unexpectedly acquired lines");
        println!(
            "{}",
            json!({"kind":"correctness","workload":name,"bytes":text.len(),
            "comparisons":comparisons,"all_equal":true,"old_snapshot_unchanged":true,
            "template_creation":creation,"interaction_exhaustive":name=="complex-small"})
        );
        for sample in 0..SAMPLES + WARMUP {
            let width = widths[sample % 4]; // resize cycle; extremes above are semantic cases.
            let alignment = aligns[sample % 5];
            // Alternate order to reduce systematic warm/cache bias.
            for ordinal in 0..2 {
                let reuse = (sample + ordinal) % 2 == 0;
                let total = Instant::now();
                let (mut layout, prepare) = timed(|| {
                    if reuse {
                        template.clone()
                    } else {
                        shape(&mut fc, &mut lc, &text, split)
                    }
                });
                let (_, wrapping) = timed(|| layout.break_all_lines(width));
                let (_, alignment_cost) =
                    timed(|| layout.align(width, alignment, AlignmentOptions::default()));
                let (output, exporting) = timed(|| export(&layout));
                // Provider takes ownership of Layout; no second Layout clone is necessary.
                let (provider, provider_cost) = timed(|| (layout, text.len()));
                black_box(&provider);
                black_box(&output);
                let total_ns = total.elapsed().as_nanos() as u64;
                if sample >= WARMUP {
                    println!(
                        "{}",
                        json!({"kind":"sample","workload":name,"sample":sample-WARMUP,
                        "path":if reuse {"clone-rewrap"} else {"fresh"},"width":width,
                        "prepare":prepare,"wrap":wrapping,"align":alignment_cost,"export":exporting,
                        "provider_move":provider_cost,"total_ns":total_ns,
                        "glyphs":output.glyphs.len(),"lines":output.lines.len()})
                    );
                }
            }
        }
    }
}
