//! Call-local clone comparison. The reference reproduces the previous request
//! storage and derived clone; it is not a frozen old-engine end-to-end baseline.
use novadraw::{
    figure::LayoutDirection,
    graphics::Color,
    text::{
        FontDescriptor, ParagraphAlignment, ParagraphDirection, StyledTextRequest, TextConstraints,
        TextInlineBox, TextStyle, TextStyleSpan,
    },
};
use std::{hint::black_box, sync::Arc, time::Instant};

const SAMPLES: usize = 400;
const CLONES: usize = 64;

#[derive(Clone)]
#[allow(dead_code)]
struct PreviousRequest {
    text: Arc<str>,
    default_style: TextStyle,
    spans: Vec<TextStyleSpan>,
    boxes: Vec<TextInlineBox>,
    constraints: TextConstraints,
    direction: ParagraphDirection,
    layout_direction: LayoutDirection,
    alignment: ParagraphAlignment,
    wrap: bool,
}

fn elapsed(mut operation: impl FnMut()) -> f64 {
    let start = Instant::now();
    for _ in 0..CLONES {
        operation();
    }
    start.elapsed().as_nanos() as f64 / CLONES as f64
}

fn report(bytes: usize, spans: usize, mode: &str, mut samples: Vec<f64>) {
    samples.sort_by(f64::total_cmp);
    println!(
        "{bytes},{spans},{mode},{SAMPLES},{CLONES},{:.2},{:.2},{:.2}",
        samples[SAMPLES / 2],
        samples[SAMPLES * 95 / 100],
        samples[SAMPLES * 99 / 100]
    );
}

fn main() {
    println!("text_bytes,spans,mode,samples,clones_per_sample,p50_ns,p95_ns,p99_ns");
    for (bytes, span_count) in [(1280, 1), (51232, 40)] {
        let style = TextStyle::new(FontDescriptor::default(), Color::BLACK);
        let text = "a".repeat(bytes);
        let spans = (0..span_count)
            .map(|index| TextStyleSpan {
                range: index * bytes / span_count..(index + 1) * bytes / span_count,
                style: style.clone(),
            })
            .collect::<Vec<_>>();
        let reference = PreviousRequest {
            text: text.clone().into(),
            default_style: style.clone(),
            spans: spans.clone(),
            boxes: vec![],
            constraints: TextConstraints::UNBOUNDED,
            direction: ParagraphDirection::Auto,
            layout_direction: LayoutDirection::default(),
            alignment: ParagraphAlignment::Leading,
            wrap: true,
        };
        let current =
            StyledTextRequest::new(text, style, spans, vec![], TextConstraints::UNBOUNDED).unwrap();
        assert_eq!(reference.text.as_ref(), current.text());
        assert_eq!(reference.spans, current.spans());
        for _ in 0..CLONES {
            black_box(reference.clone());
            black_box(current.clone());
        }
        let mut previous = Vec::with_capacity(SAMPLES);
        let mut shared = Vec::with_capacity(SAMPLES);
        for sample in 0..SAMPLES {
            let mut old = || {
                black_box(reference.clone());
            };
            let mut new = || {
                black_box(current.clone());
            };
            if sample % 2 == 0 {
                previous.push(elapsed(&mut old));
                shared.push(elapsed(&mut new));
            } else {
                shared.push(elapsed(&mut new));
                previous.push(elapsed(&mut old));
            }
        }
        report(bytes, span_count, "previous_storage_clone", previous);
        report(bytes, span_count, "shared_request_clone", shared);
    }
}
