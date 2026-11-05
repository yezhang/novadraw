//! Document-backed rich text, independent of the Flow contributor tree.
//! Source changes use Core typed updates; drawing uses Runtime's text service.

mod cache;
mod document;
mod figure;
mod index;
mod layout;

pub use document::{
    DocumentError, DocumentLimits, DocumentVersion, ParagraphId, ParagraphInput,
    RichTextDocumentBuilder, RichTextDocumentSnapshot, RichTextParagraph,
};
pub use figure::{
    RICH_TEXT_DOCUMENT, ReplaceRichTextDocument, RichTextContentFigure, RichTextFigure,
    RichTextHandle,
};
pub use index::ParagraphHeightIndex;
pub use layout::{RichTextLayoutSnapshot, RichTextPosition};
