//! Backend-neutral rendering integration.

pub use novadraw_render::{
    BackendCapabilities, DamageMode, DamageSet, FrameId, RenderBackend, RenderCapability,
    RenderOutcome, RenderSubmission, ResourceDelta, ResourceId, ResourceSync, SurfaceInfo,
    UnsupportedRenderCapability,
};

/// Recorded drawing commands and immutable image references.
pub mod command {
    pub use novadraw_render::command::{
        DEFAULT_STROKE_MITER_LIMIT, ImageData, ImageDecodeError, ImageDrawError, ImageResourceRef,
        LineCap, LineJoin, LineStyle, Path, PathOp, RenderCommand, RenderCommandKind,
    };
}

/// Frame, damage, resource synchronization, and backend session protocol.
pub mod submission {
    pub use novadraw_render::submission::{
        BackendSessionDecision, BackendSessionGate, BackendSessionId, DamageMode, DamageSet,
        FontData, FrameId, RenderSubmission, ResourceDelta, ResourceId, ResourceOp,
        ResourcePayload, ResourceSnapshot, ResourceSync, ResourceUpdate, SurfaceInfo,
    };
}

/// Text shaping and backend-neutral glyph layout.
pub mod text {
    pub use novadraw_render::text::{
        BuiltinFont, FontDescriptor, FontFaceRef, FontStyle, GlyphPaint, GlyphRun,
        ParleyTextEngine, PositionedGlyph, TextConstraints, TextEngine, TextError, TextLayout,
        TextLayoutEngine, TextLayoutKey, TextLayoutParts, TextLineMetrics,
    };
}
