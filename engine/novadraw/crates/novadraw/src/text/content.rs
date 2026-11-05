use std::{
    error::Error,
    fmt,
    sync::{Arc, Mutex, OnceLock},
};

use super::{TextError, TextStyle, TextStyleSpan};

/// Logical storage admission failure. Units account for owned capacities and metadata,
/// not allocator overhead or process RSS.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TextContentError {
    Capacity {
        retained: usize,
        requested: usize,
        limit: usize,
    },
    AccountingOverflow,
    LedgerPoisoned,
}
impl fmt::Display for TextContentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "text content admission failed: {self:?}")
    }
}
impl Error for TextContentError {}
impl From<TextContentError> for TextError {
    fn from(error: TextContentError) -> Self {
        Self::Content(Box::new(error))
    }
}

/// Unique live content in one storage domain. Clones and requests share the charge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextContentUsage {
    pub retained_units: usize,
    pub content_count: usize,
    pub limit: usize,
}
impl TextContentUsage {
    pub fn is_over_limit(self) -> bool {
        self.retained_units > self.limit
    }
}

/// Instance-owned content admission. Clones share limits and the live-content ledger.
/// Dropping the store never invalidates old content or its accounting lease.
#[derive(Clone, Debug)]
pub struct TextContentStore(Arc<Mutex<TextContentUsage>>);
impl TextContentStore {
    /// Creates an empty domain with an explicit logical retention limit.
    pub fn new(limit: usize) -> Self {
        Self(Arc::new(Mutex::new(TextContentUsage {
            retained_units: 0,
            content_count: 0,
            limit,
        })))
    }
    pub fn usage(&self) -> Result<TextContentUsage, TextContentError> {
        self.0
            .lock()
            .map(|usage| *usage)
            .map_err(|_| TextContentError::LedgerPoisoned)
    }
    /// Shrinking a limit preserves all existing leases. Positive new charges are
    /// refused until releases or a larger limit make admission possible again.
    pub fn set_limit(&self, limit: usize) -> Result<(), TextContentError> {
        self.0
            .lock()
            .map_err(|_| TextContentError::LedgerPoisoned)?
            .limit = limit;
        Ok(())
    }
    /// Internal metadata and imported-backing reservation in this same domain.
    pub(crate) fn reserve(&self, units: usize) -> Result<TextContentLease, TextContentError> {
        let mut usage = self
            .0
            .lock()
            .map_err(|_| TextContentError::LedgerPoisoned)?;
        let retained = usage
            .retained_units
            .checked_add(units)
            .ok_or(TextContentError::AccountingOverflow)?;
        if units > 0 && retained > usage.limit {
            return Err(TextContentError::Capacity {
                retained: usage.retained_units,
                requested: units,
                limit: usage.limit,
            });
        }
        usage.retained_units = retained;
        Ok(TextContentLease(Arc::new(Reservation {
            units,
            ledger: self.0.clone(),
        })))
    }
    pub fn create(
        &self,
        text: String,
        default_style: TextStyle,
        spans: Vec<TextStyleSpan>,
    ) -> Result<StyledTextContent, TextError> {
        validate_spans(&text, &spans)?;
        // The incoming owned buffers already exist. Admission precedes allocating
        // the retained record; rejection returns no unaccounted content handle.
        let units = content_units(&text, &default_style, &spans)
            .ok_or(TextContentError::AccountingOverflow)?;
        let mut usage = self
            .0
            .lock()
            .map_err(|_| TextContentError::LedgerPoisoned)?;
        let retained = usage
            .retained_units
            .checked_add(units)
            .ok_or(TextContentError::AccountingOverflow)?;
        let count = usage
            .content_count
            .checked_add(1)
            .ok_or(TextContentError::AccountingOverflow)?;
        if retained > usage.limit {
            return Err(TextContentError::Capacity {
                retained: usage.retained_units,
                requested: units,
                limit: usage.limit,
            }
            .into());
        }
        usage.retained_units = retained;
        usage.content_count = count;
        drop(usage);
        Ok(StyledTextContent(Arc::new(Content {
            text,
            default_style,
            spans,
            units,
            analysis_identity: OnceLock::new(),
            ledger: Some(self.0.clone()),
        })))
    }
}

struct Reservation {
    units: usize,
    ledger: Arc<Mutex<TextContentUsage>>,
}
impl Drop for Reservation {
    fn drop(&mut self) {
        self.ledger
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .retained_units -= self.units;
    }
}
#[derive(Clone)]
pub(crate) struct TextContentLease(Arc<Reservation>);
impl TextContentLease {
    pub(crate) fn units(&self) -> usize {
        self.0.units
    }
}

struct Content {
    text: String,
    default_style: TextStyle,
    spans: Vec<TextStyleSpan>,
    units: usize,
    analysis_identity: OnceLock<super::shaping::TextAnalysisBackingIdentity>,
    ledger: Option<Arc<Mutex<TextContentUsage>>>,
}
impl Drop for Content {
    fn drop(&mut self) {
        // Internal release only: no user callbacks, Runtime access or fallible work.
        let Some(ledger) = &self.ledger else {
            return;
        };
        let mut usage = ledger.lock().unwrap_or_else(|poison| poison.into_inner());
        usage.retained_units -= self.units;
        usage.content_count -= 1;
    }
}

/// Checked immutable paragraph content. Text, styles and spans share one lease.
/// Width, paragraph options and measured inline boxes belong to the request.
#[derive(Clone)]
pub struct StyledTextContent(Arc<Content>);
impl StyledTextContent {
    /// Owned compatibility entry point with an independent, unbounded domain.
    /// Use `TextContentStore::create` when a caller needs bounded retained storage.
    pub fn new(
        text: impl Into<String>,
        default_style: TextStyle,
        spans: Vec<TextStyleSpan>,
    ) -> Result<Self, TextError> {
        let text = text.into();
        validate_spans(&text, &spans)?;
        let units = content_units(&text, &default_style, &spans)
            .ok_or(TextContentError::AccountingOverflow)?;
        // An independent unlimited owner needs no mutable admission ledger.
        // Its charge and lifetime are represented by the single content record.
        Ok(Self(Arc::new(Content {
            text,
            default_style,
            spans,
            units,
            analysis_identity: OnceLock::new(),
            ledger: None,
        })))
    }
    pub fn text(&self) -> &str {
        &self.0.text
    }
    pub fn default_style(&self) -> &TextStyle {
        &self.0.default_style
    }
    pub fn spans(&self) -> &[TextStyleSpan] {
        &self.0.spans
    }
    /// Charge for this complete content backing in its owning domain.
    pub fn retained_units(&self) -> usize {
        self.0.units
    }
    pub fn usage(&self) -> Result<TextContentUsage, TextContentError> {
        let Some(ledger) = &self.0.ledger else {
            return Ok(TextContentUsage {
                retained_units: self.0.units,
                content_count: 1,
                limit: usize::MAX,
            });
        };
        ledger
            .lock()
            .map(|usage| *usage)
            .map_err(|_| TextContentError::LedgerPoisoned)
    }
    pub(crate) fn analysis_backing(&self) -> super::shaping::TextAnalysisBacking {
        super::shaping::TextAnalysisBacking {
            identity: *self.0.analysis_identity.get_or_init(Default::default),
            retained_bytes: self.0.units,
        }
    }
}
impl fmt::Debug for StyledTextContent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StyledTextContent")
            .field("text", &self.text())
            .field("default_style", self.default_style())
            .field("spans", &self.spans())
            .finish()
    }
}
impl PartialEq for StyledTextContent {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
            || (self.text() == other.text()
                && self.default_style() == other.default_style()
                && self.spans() == other.spans())
    }
}

fn validate_spans(text: &str, spans: &[TextStyleSpan]) -> Result<(), TextError> {
    let mut end = 0;
    for span in spans {
        if span.range.start < end
            || span.range.start >= span.range.end
            || span.range.end > text.len()
            || !text.is_char_boundary(span.range.start)
            || !text.is_char_boundary(span.range.end)
        {
            return Err(TextError::InvalidStyleRange);
        }
        end = span.range.end;
    }
    Ok(())
}

#[allow(clippy::ptr_arg)] // Owned capacity, rather than slice length, is the charged storage.
fn content_units(text: &String, style: &TextStyle, spans: &Vec<TextStyleSpan>) -> Option<usize> {
    let mut units = std::mem::size_of::<Content>()
        .checked_add(2 * std::mem::size_of::<usize>())?
        .checked_add(text.capacity())?
        .checked_add(style.font().family_capacity())?
        .checked_add(
            spans
                .capacity()
                .checked_mul(std::mem::size_of::<TextStyleSpan>())?,
        )?;
    for span in spans {
        units = units.checked_add(span.style.font().family_capacity())?;
    }
    Some(units)
}
