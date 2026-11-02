//! Owned diagnostics for Editor extension boundaries.

use std::{any::Any, error::Error, fmt, rc::Rc};

/// Classification of a caught panic payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PanicPayloadKind {
    /// An owned String or static string slice.
    Text,
    /// An arbitrary payload, never formatted by the Editor.
    NonText,
}

/// Diagnostic retained without converting a panic into an Error source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PanicDiagnostic {
    kind: PanicPayloadKind,
    message: Option<String>,
    location: &'static str,
    call_site: &'static std::panic::Location<'static>,
}

impl PanicDiagnostic {
    #[track_caller]
    pub(crate) fn capture(payload: &(dyn Any + Send), location: &'static str) -> Self {
        Self::capture_at(payload, location, std::panic::Location::caller())
    }

    pub(crate) fn capture_at(
        payload: &(dyn Any + Send),
        location: &'static str,
        call_site: &'static std::panic::Location<'static>,
    ) -> Self {
        let message = payload.downcast_ref::<String>().cloned().or_else(|| {
            payload
                .downcast_ref::<&'static str>()
                .map(|text| (*text).to_owned())
        });
        Self {
            kind: if message.is_some() {
                PanicPayloadKind::Text
            } else {
                PanicPayloadKind::NonText
            },
            message,
            location,
            call_site,
        }
    }

    /// Returns the payload classification.
    pub const fn kind(&self) -> PanicPayloadKind {
        self.kind
    }

    /// Returns the original text, if the payload was textual.
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    /// Returns the Editor callback boundary that caught the panic.
    pub const fn location(&self) -> &'static str {
        self.location
    }

    /// Returns the source location of the Editor call boundary.
    pub const fn call_site(&self) -> &'static std::panic::Location<'static> {
        self.call_site
    }
}

impl fmt::Display for PanicDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message().unwrap_or("non-text panic payload"))
    }
}

#[derive(Clone)]
pub(crate) struct OwnedError(Rc<dyn Error>);

impl OwnedError {
    pub(crate) fn new(error: impl Error + 'static) -> Self {
        Self(Rc::new(error))
    }
    pub(crate) fn get(&self) -> &(dyn Error + 'static) {
        self.0.as_ref()
    }
}

impl fmt::Debug for OwnedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("OwnedError(..)")
    }
}
