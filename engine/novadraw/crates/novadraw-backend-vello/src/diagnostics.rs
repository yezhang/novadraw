//! Optional synchronous backend tracing. The host owns the SDK session.
#[cfg(all(feature = "perfetto", not(target_arch = "wasm32")))]
use perfetto_sdk::track_event::{ScopeGuard, TrackEventType};

#[cfg(all(feature = "perfetto", not(target_arch = "wasm32")))]
perfetto_sdk::track_event_categories! {
    pub mod perfetto_te_ns {
        ("novadraw.backend", "Vello CPU submission stages", []),
        ("novadraw.glyph", "Per-run glyph encoding", []),
    }
}

#[cfg(all(feature = "perfetto", not(target_arch = "wasm32")))]
pub fn register() -> Result<(), String> {
    perfetto_te_ns::register().map_err(|error| error.to_string())
}

pub(crate) struct Scope {
    _thread: std::marker::PhantomData<std::rc::Rc<()>>,
    #[cfg(all(feature = "perfetto", not(target_arch = "wasm32")))]
    _guard: ScopeGuard<fn()>,
}

#[cfg(not(all(feature = "perfetto", not(target_arch = "wasm32"))))]
impl Drop for Scope {
    fn drop(&mut self) {}
}

pub(crate) fn scope(name: &'static std::ffi::CStr) -> Scope {
    #[cfg(all(feature = "perfetto", not(target_arch = "wasm32")))]
    {
        perfetto_sdk::track_event!(
            "novadraw.backend",
            TrackEventType::SliceBegin(name.as_ptr())
        );
        Scope {
            _thread: Default::default(),
            _guard: ScopeGuard::new(
                (|| perfetto_sdk::track_event!("novadraw.backend", TrackEventType::SliceEnd))
                    as fn(),
            ),
        }
    }
    #[cfg(not(all(feature = "perfetto", not(target_arch = "wasm32"))))]
    {
        let _ = name;
        Scope {
            _thread: Default::default(),
        }
    }
}

pub(crate) fn glyph_scope() -> Scope {
    #[cfg(all(feature = "perfetto", not(target_arch = "wasm32")))]
    {
        perfetto_sdk::track_event!(
            "novadraw.glyph",
            TrackEventType::SliceBegin(c"backend-glyph-run".as_ptr())
        );
        Scope {
            _thread: Default::default(),
            _guard: ScopeGuard::new(
                (|| perfetto_sdk::track_event!("novadraw.glyph", TrackEventType::SliceEnd))
                    as fn(),
            ),
        }
    }
    #[cfg(not(all(feature = "perfetto", not(target_arch = "wasm32"))))]
    Scope {
        _thread: Default::default(),
    }
}
