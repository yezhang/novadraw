//! Figure extension traits, built-in figures, borders, and styles.

pub use novadraw_scene::{
    AccessibleFigure, Alignment, AsAny, Border, BorderedFigure, Bounded, ButtonFigure,
    ChildClippingStrategy, ChildPolicy, ChildTransform, ClickableBehavior, ClickableFigure,
    ClickableKind, ClickableModel, ClickableSnapshot, ClickableVisualState, Direction,
    EllipseFigure, Figure, FigureContainer, FigureLifecycle, FigureLifecycleContext,
    FigureMeasurement, Freeform, HitParticipation, ImageDisplayState, ImageFigure, LabelFigure,
    Layer, MeasureConstraints, MeasureConstraintsError, PointListFigureBehavior, PolygonFigure,
    PolylineFigure, RectangleFigure, ResolvedStyle, RoundedRectangleFigure, Shape,
    ShapeMutationError, TextPlacement, ToggleFigure, TriangleFigure, WidgetError,
};
pub use novadraw_scene::{CursorIcon, FigureStyle};

/// Built-in border implementations and border extension traits.
pub mod border {
    pub use novadraw_scene::{
        BevelBorder, BevelStyle, Border, BorderStyle, CompoundBorder, EtchedBorder, LineBorder,
        MarginBorder, RectangleBorder, TitleBarBorder,
    };
}
