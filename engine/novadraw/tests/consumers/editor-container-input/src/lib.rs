//! Platform input verification consumer; no production fault injection.

#[allow(dead_code)]
mod fixture {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../crates/novadraw-editor/tests/p2_e03_container_editing_contract.rs"
    ));
    include!("bridge.rs");
}

pub use fixture::{InputProbe, ProbeSnapshot, VisualRect};
