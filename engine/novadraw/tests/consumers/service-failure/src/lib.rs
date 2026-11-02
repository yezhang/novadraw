use novadraw::runtime::Runtime;

#[cfg(feature = "consume-inspector")]
pub mod inspection;

#[cfg(feature = "consume-connections")]
#[unsafe(no_mangle)]
pub extern "C" fn connection_registration() -> bool {
    let mut runtime = Runtime::empty();
    let anchor =
        runtime
            .connections()
            .register_anchor(Box::new(novadraw::connection::XYAnchor::new(
                novadraw::geometry::Point::new(0.0, 0.0),
                novadraw::connection::CoordinateSpace::LogicalSurface,
            )));
    anchor.is_ok()
        && runtime
            .connections()
            .register_router(Box::new(novadraw::connection::DirectRouter))
            .is_ok()
}

#[cfg(all(test, feature = "consume-connections"))]
mod connection_contracts {
    include!("../../../../crates/novadraw/tests/p2_s09_connection_registration.rs");
}

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
    let subscription = inspector.attach(&mut runtime).unwrap();
    runtime.frames().stabilize_for_query().unwrap();
    let count = inspector.events().unwrap().len();
    subscription.detach(&mut runtime).unwrap();
    count
}

#[cfg(all(test, feature = "consume-animation"))]
mod contracts {
    include!("../../../../crates/novadraw/tests/p2_s09_animation_failure.rs");
}
