//! Input-only overhead sampling; does not measure shaping or Core width reuse.
use harfrust::{BufferOperation, UnicodeBuffer};
use std::{hint::black_box, time::Instant};

fn main() {
    const SAMPLES: usize = 1000;
    let text = "office é a\u{301} 中文 العربية 🙂\n".repeat(1024);
    let count = text.chars().count();
    for warm in [false, true] {
        for path in ["ordinary", "admitted", "reject-append"] {
            let mut buffer = UnicodeBuffer::new();
            if warm {
                assert!(buffer.reserve(count));
                buffer.clear();
            }
            // Interleave paths in a separate loop invocation if comparing CPU
            // latency rigorously. These sequential samples are local overhead evidence.
            for index in 0..SAMPLES {
                let mut calls = 0usize;
                let mut work = 0usize;
                let mut allocation = 0usize;
                let start = Instant::now();
                if !warm {
                    buffer = UnicodeBuffer::new();
                }
                let result = match path {
                    "ordinary" => {
                        buffer.push_str(black_box(&text));
                        Ok(())
                    }
                    _ => buffer.try_push_str_admitted(black_box(&text), |operation, cost| {
                        calls += 1;
                        if path == "reject-append" && operation == BufferOperation::Append {
                            return Err(());
                        }
                        work += cost.work;
                        allocation += cost.allocation;
                        Ok(())
                    }),
                };
                let elapsed = start.elapsed().as_nanos();
                let rejected = result.is_err();
                assert_eq!(rejected, path == "reject-append");
                assert_eq!(buffer.len(), if rejected { 0 } else { count });
                black_box(&buffer);
                println!(
                    "{{\"path\":\"{path}\",\"warm\":{warm},\"sample\":{index},\
                    \"elapsed_ns\":{elapsed},\"bytes\":{},\"characters\":{count},\
                    \"calls\":{calls},\"work\":{work},\"allocation\":{allocation},\
                    \"rejected\":{rejected}}}",
                    text.len()
                );
                buffer.clear();
            }
        }
    }
}
