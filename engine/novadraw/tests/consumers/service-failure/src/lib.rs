use novadraw::runtime::Runtime;

#[unsafe(no_mangle)]
pub extern "C" fn empty_owner() -> bool {
    Runtime::empty().is_faulted()
}

#[cfg(feature = "consume-animation")]
#[unsafe(no_mangle)]
pub extern "C" fn animation_modes(change: bool, updates: usize) -> u64 {
    let mut runtime = Runtime::empty();
    mode_updates(&mut runtime, change, updates)
}

#[cfg(feature = "consume-animation")]
pub fn mode_updates(runtime: &mut Runtime, change: bool, updates: usize) -> u64 {
    use novadraw::animation::AnimationMode;
    for index in 0..updates {
        let mode = if change && index % 2 == 0 {
            AnimationMode::Disabled
        } else {
            AnimationMode::Enabled
        };
        // This identical probe compiles against the old bool and the new Result API.
        // The public contract tests below separately require successful Results.
        let _ = std::hint::black_box(runtime.animation().timeline().set_mode(mode));
    }
    runtime
        .animation()
        .timeline()
        .sample_version()
        .active_revision
}

#[cfg(feature = "consume-inspector")]
#[unsafe(no_mangle)]
pub extern "C" fn inspection_events() -> usize {
    use novadraw_inspector::FigureInspector;
    let inspector = FigureInspector::new(std::num::NonZeroUsize::new(16).unwrap());
    let mut runtime = Runtime::empty();
    let _subscription = inspector.attach(&mut runtime);
    inspector.events().len()
}

#[cfg(test)]
mod contracts {
    include!("../../../../crates/novadraw/tests/p2_s09_animation_failure.rs");
}
