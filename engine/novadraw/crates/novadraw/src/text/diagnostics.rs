/// CPU stage instrumentation shared by built-in and external text engines.
/// The optional native Perfetto SDK owns timestamps and event storage.
#[derive(Clone, Default)]
pub struct TextLayoutDiagnostics {
    pub(crate) namespace: Option<crate::identity::RuntimeNamespace>,
}

impl TextLayoutDiagnostics {
    /// Starts a synchronous stage when the host has enabled the Perfetto category.
    pub fn time(&self, stage: &'static str) -> TextLayoutTimingScope {
        #[cfg(all(feature = "perfetto", not(target_arch = "wasm32")))]
        {
            native::scope(self, stage)
        }
        #[cfg(not(all(feature = "perfetto", not(target_arch = "wasm32"))))]
        {
            let _ = (self.namespace, stage);
            TextLayoutTimingScope {}
        }
    }

    pub(crate) fn request(&self, details: impl FnOnce() -> String) {
        #[cfg(all(feature = "perfetto", not(target_arch = "wasm32")))]
        native::request(self, details);
        #[cfg(not(all(feature = "perfetto", not(target_arch = "wasm32"))))]
        let _ = details;
    }
}

/// Ends the SDK slice on return, including error returns; cannot cross threads.
#[must_use]
pub struct TextLayoutTimingScope {
    #[cfg(all(feature = "perfetto", not(target_arch = "wasm32")))]
    _guard: Option<perfetto_sdk::track_event::ScopeGuard<fn()>>,
    #[cfg(all(feature = "perfetto", not(target_arch = "wasm32")))]
    _thread: std::marker::PhantomData<std::rc::Rc<()>>,
}

/// Registers Core categories after the host initializes the native SDK producer.
#[cfg(all(feature = "perfetto", not(target_arch = "wasm32")))]
pub fn register_perfetto_categories() -> Result<(), String> {
    native::perfetto_te_ns::register().map_err(|error| error.to_string())
}

#[cfg(all(feature = "perfetto", not(target_arch = "wasm32")))]
mod native {
    use super::*;
    use perfetto_sdk::track_event::{EventContext, ScopeGuard, TrackEventDebugArg, TrackEventType};
    use std::ffi::CString;

    perfetto_sdk::track_event_categories! {
        pub mod perfetto_te_ns {
            ("novadraw.phase", "Core CPU stages", []),
            ("novadraw.request", "Complete Flow request diagnostics", []),
        }
    }

    fn identity(ctx: &mut EventContext, diagnostics: &TextLayoutDiagnostics) {
        if let Some(namespace) = diagnostics.namespace {
            ctx.add_debug_arg(
                "runtime",
                TrackEventDebugArg::String(&namespace.as_uuid().to_string()),
            );
        }
    }

    pub(super) fn scope(diagnostics: &TextLayoutDiagnostics, stage: &str) -> TextLayoutTimingScope {
        let guard = if perfetto_sdk::track_event_category_enabled!("novadraw.phase") {
            // Stage names are diagnostic labels supplied by external engines too.
            let name = CString::new(stage.replace('\0', "\u{fffd}")).unwrap();
            perfetto_sdk::track_event!(
                "novadraw.phase",
                TrackEventType::SliceBegin(name.as_ptr()),
                |ctx: &mut EventContext| {
                    identity(ctx, diagnostics);
                }
            );
            Some(ScopeGuard::new(
                (|| perfetto_sdk::track_event!("novadraw.phase", TrackEventType::SliceEnd))
                    as fn(),
            ))
        } else {
            None
        };
        TextLayoutTimingScope {
            _guard: guard,
            _thread: Default::default(),
        }
    }

    pub(super) fn request(diagnostics: &TextLayoutDiagnostics, details: impl FnOnce() -> String) {
        perfetto_sdk::track_event!(
            "novadraw.request",
            TrackEventType::Instant(c"flow-layout-attempt".as_ptr()),
            |ctx: &mut EventContext| {
                identity(ctx, diagnostics);
                ctx.add_debug_arg("request", TrackEventDebugArg::String(&details()));
            }
        );
    }
}
