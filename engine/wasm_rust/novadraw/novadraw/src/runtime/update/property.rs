//! Typed property identity and heterogeneous diagnostic values.

use std::any::{Any, TypeId, type_name};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::sync::Arc;

use crate::Color;
use crate::figure::{
    Alignment, Direction, FlowPage, FlowWrapping, PolygonScaleMode, TextPlacement,
};
use crate::geometry::{Dimension, Point, Rectangle};
use crate::graph::FigureId;
use crate::render::{LineJoin, StrokeStyle};
use crate::runtime::resource::ImageId;
use crate::style::CursorIcon;

/// Stable, typed identity for one property value family.
pub struct PropertyKey<V> {
    namespace: &'static str,
    name: &'static str,
    marker: PhantomData<fn() -> V>,
}

impl<V> PropertyKey<V> {
    /// Creates a property key with a non-empty namespace and stable display name.
    pub const fn new(namespace: &'static str, name: &'static str) -> Self {
        assert!(
            !namespace.is_empty(),
            "property namespace must not be empty"
        );
        assert!(!name.is_empty(), "property name must not be empty");
        Self {
            namespace,
            name,
            marker: PhantomData,
        }
    }

    /// Returns the stable property namespace.
    pub const fn namespace(self) -> &'static str {
        self.namespace
    }

    /// Returns the stable diagnostic name.
    pub const fn name(self) -> &'static str {
        self.name
    }

    /// Erases the generic value type while preserving its runtime identity.
    pub fn erase(self) -> ErasedPropertyKey
    where
        V: 'static,
    {
        ErasedPropertyKey {
            namespace: self.namespace,
            name: self.name,
            value_type: TypeId::of::<V>(),
            value_type_name: type_name::<V>(),
        }
    }
}

impl<V> Clone for PropertyKey<V> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<V> Copy for PropertyKey<V> {}

impl<V> fmt::Debug for PropertyKey<V> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PropertyKey")
            .field("namespace", &self.namespace)
            .field("name", &self.name)
            .field("value_type", &type_name::<V>())
            .finish()
    }
}

impl<V> PartialEq for PropertyKey<V> {
    fn eq(&self, other: &Self) -> bool {
        self.namespace == other.namespace && self.name == other.name
    }
}

impl<V> Eq for PropertyKey<V> {}

impl<V> Hash for PropertyKey<V> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.namespace.hash(state);
        self.name.hash(state);
    }
}

/// Type-erased property identity stored in heterogeneous journals.
#[derive(Clone, Copy, Eq)]
pub struct ErasedPropertyKey {
    namespace: &'static str,
    name: &'static str,
    value_type: TypeId,
    value_type_name: &'static str,
}

impl ErasedPropertyKey {
    /// Returns the stable property namespace.
    pub const fn namespace(self) -> &'static str {
        self.namespace
    }

    /// Returns the stable diagnostic name.
    pub const fn name(self) -> &'static str {
        self.name
    }

    /// Returns the Rust value type name for diagnostics.
    pub const fn value_type_name(self) -> &'static str {
        self.value_type_name
    }

    /// Returns whether this identity was erased from the supplied typed key.
    pub fn is<V: 'static>(self, key: PropertyKey<V>) -> bool {
        self == key.erase()
    }
}

impl fmt::Debug for ErasedPropertyKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ErasedPropertyKey")
            .field("namespace", &self.namespace)
            .field("name", &self.name)
            .field("value_type", &self.value_type_name)
            .finish()
    }
}

impl PartialEq for ErasedPropertyKey {
    fn eq(&self, other: &Self) -> bool {
        self.namespace == other.namespace
            && self.name == other.name
            && self.value_type == other.value_type
    }
}

impl Hash for ErasedPropertyKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.namespace.hash(state);
        self.name.hash(state);
        self.value_type.hash(state);
    }
}

/// Heterogeneous diagnostic value retained by property journals.
#[derive(Debug, Clone, PartialEq)]
pub enum PropertyValue {
    /// Boolean value.
    Bool(bool),
    /// Floating-point number.
    Number(f64),
    /// Color value.
    Color(Color),
    /// Cursor value.
    Cursor(CursorIcon),
    /// Point value.
    Point(Point),
    /// Point-list value.
    PointList(Vec<Point>),
    /// Size value.
    Size(Dimension),
    /// Rectangle value.
    Rectangle(Rectangle),
    /// Text or formatted domain diagnostic.
    Text(String),
    /// Figure identity.
    Figure(Option<FigureId>),
    /// Absent optional value.
    None,
}

/// Converts one typed property value into its heterogeneous diagnostic form.
pub trait PropertyValueType: Clone + PartialEq + Send + Sync + 'static {
    /// Converts this value into a journal diagnostic value.
    fn into_property_value(self) -> PropertyValue;
}

/// Marker for values that may be used with state-change animation triggers.
pub trait DiscretePropertyValue: PropertyValueType {}

impl PropertyValueType for bool {
    fn into_property_value(self) -> PropertyValue {
        PropertyValue::Bool(self)
    }
}

impl DiscretePropertyValue for bool {}

impl PropertyValueType for f64 {
    fn into_property_value(self) -> PropertyValue {
        PropertyValue::Number(self)
    }
}

impl PropertyValueType for Color {
    fn into_property_value(self) -> PropertyValue {
        PropertyValue::Color(self)
    }
}

impl PropertyValueType for CursorIcon {
    fn into_property_value(self) -> PropertyValue {
        PropertyValue::Cursor(self)
    }
}

impl DiscretePropertyValue for CursorIcon {}

impl PropertyValueType for Point {
    fn into_property_value(self) -> PropertyValue {
        PropertyValue::Point(self)
    }
}

impl PropertyValueType for Vec<Point> {
    fn into_property_value(self) -> PropertyValue {
        PropertyValue::PointList(self)
    }
}

impl PropertyValueType for Dimension {
    fn into_property_value(self) -> PropertyValue {
        PropertyValue::Size(self)
    }
}

impl PropertyValueType for Rectangle {
    fn into_property_value(self) -> PropertyValue {
        PropertyValue::Rectangle(self)
    }
}

impl PropertyValueType for String {
    fn into_property_value(self) -> PropertyValue {
        PropertyValue::Text(self)
    }
}

impl PropertyValueType for FigureId {
    fn into_property_value(self) -> PropertyValue {
        PropertyValue::Figure(Some(self))
    }
}

macro_rules! diagnostic_text_value {
    ($($value:ty),+ $(,)?) => {
        $(
            impl PropertyValueType for $value {
                fn into_property_value(self) -> PropertyValue {
                    PropertyValue::Text(format!("{self:?}"))
                }
            }
        )+
    };
}

diagnostic_text_value!(
    Alignment,
    Direction,
    FlowPage,
    FlowWrapping,
    PolygonScaleMode,
    TextPlacement,
    LineJoin,
    StrokeStyle,
    ImageId,
    (Alignment, Alignment),
);

impl<T> PropertyValueType for Option<T>
where
    T: PropertyValueType,
{
    fn into_property_value(self) -> PropertyValue {
        self.map_or(PropertyValue::None, PropertyValueType::into_property_value)
    }
}

impl DiscretePropertyValue for Option<FigureId> {}

/// A property change whose key and values share the same compile-time type.
#[derive(Clone, Debug, PartialEq)]
pub struct TypedPropertyChange<V> {
    figure_id: FigureId,
    property: PropertyKey<V>,
    old_value: V,
    new_value: V,
}

impl<V> TypedPropertyChange<V> {
    /// Creates a typed property change.
    pub fn new(figure_id: FigureId, property: PropertyKey<V>, old_value: V, new_value: V) -> Self {
        Self {
            figure_id,
            property,
            old_value,
            new_value,
        }
    }

    /// Returns the changed Figure.
    pub const fn figure_id(&self) -> FigureId {
        self.figure_id
    }

    /// Returns the typed property identity.
    pub const fn property(&self) -> PropertyKey<V> {
        self.property
    }

    /// Returns the old typed value.
    pub const fn old_value(&self) -> &V {
        &self.old_value
    }

    /// Returns the new typed value.
    pub const fn new_value(&self) -> &V {
        &self.new_value
    }
}

impl<V> TypedPropertyChange<V>
where
    V: PropertyValueType,
{
    /// Erases this change for heterogeneous Runtime journals.
    pub fn erase(self) -> PropertyChangeEvent {
        let old_comparable = ComparablePropertyValue::new(self.old_value.clone());
        let new_comparable = ComparablePropertyValue::new(self.new_value.clone());
        PropertyChangeEvent {
            figure_id: self.figure_id,
            property: self.property.erase(),
            old_value: self.old_value.into_property_value(),
            new_value: self.new_value.into_property_value(),
            old_comparable,
            new_comparable,
        }
    }
}

/// Type-erased property change delivered to listeners and diagnostic consumers.
#[derive(Clone)]
pub struct PropertyChangeEvent {
    figure_id: FigureId,
    property: ErasedPropertyKey,
    old_value: PropertyValue,
    new_value: PropertyValue,
    old_comparable: ComparablePropertyValue,
    new_comparable: ComparablePropertyValue,
}

impl PropertyChangeEvent {
    /// Returns the changed Figure.
    pub const fn figure_id(&self) -> FigureId {
        self.figure_id
    }

    /// Returns the type-erased stable property identity.
    pub const fn property(&self) -> ErasedPropertyKey {
        self.property
    }

    /// Returns the old diagnostic value.
    pub const fn old_value(&self) -> &PropertyValue {
        &self.old_value
    }

    /// Returns the new diagnostic value.
    pub const fn new_value(&self) -> &PropertyValue {
        &self.new_value
    }

    pub(crate) fn is_noop(&self) -> bool {
        self.old_comparable == self.new_comparable
    }

    pub(crate) fn merge_latest(&mut self, latest: Self) {
        debug_assert_eq!(self.figure_id, latest.figure_id);
        debug_assert_eq!(self.property, latest.property);
        self.new_value = latest.new_value;
        self.new_comparable = latest.new_comparable;
    }
}

impl fmt::Debug for PropertyChangeEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PropertyChangeEvent")
            .field("figure_id", &self.figure_id)
            .field("property", &self.property)
            .field("old_value", &self.old_value)
            .field("new_value", &self.new_value)
            .finish()
    }
}

impl PartialEq for PropertyChangeEvent {
    fn eq(&self, other: &Self) -> bool {
        self.figure_id == other.figure_id
            && self.property == other.property
            && self.old_comparable == other.old_comparable
            && self.new_comparable == other.new_comparable
    }
}

type PropertyAny = dyn Any + Send + Sync;
type PropertyEquals = fn(&PropertyAny, &PropertyAny) -> bool;

#[derive(Clone)]
struct ComparablePropertyValue {
    value: Arc<PropertyAny>,
    equals: PropertyEquals,
}

impl ComparablePropertyValue {
    fn new<V>(value: V) -> Self
    where
        V: Clone + PartialEq + Send + Sync + 'static,
    {
        Self {
            value: Arc::new(value),
            equals: |left, right| {
                left.downcast_ref::<V>()
                    .zip(right.downcast_ref::<V>())
                    .is_some_and(|(left, right)| left == right)
            },
        }
    }
}

impl PartialEq for ComparablePropertyValue {
    fn eq(&self, other: &Self) -> bool {
        (self.equals)(self.value.as_ref(), other.value.as_ref())
    }
}

const FIGURE_NAMESPACE: &str = "novadraw.figure";
const STYLE_NAMESPACE: &str = "novadraw.style";
const CONTAINER_NAMESPACE: &str = "novadraw.container";

/// Standard property keys emitted by built-in Figure and container services.
pub mod standard {
    use super::PropertyKey;
    use crate::Color;
    use crate::figure::{
        Alignment, Direction, FlowPage, FlowWrapping, PolygonScaleMode, TextPlacement,
    };
    use crate::geometry::{Dimension, Point, Rectangle};
    use crate::render::{LineJoin, StrokeStyle};
    use crate::runtime::ImageId;
    use crate::style::CursorIcon;

    /// Figure visibility.
    pub const VISIBLE: PropertyKey<bool> = PropertyKey::new(super::FIGURE_NAMESPACE, "visible");
    /// Figure enabled state.
    pub const ENABLED: PropertyKey<bool> = PropertyKey::new(super::FIGURE_NAMESPACE, "enabled");
    /// Figure focus eligibility.
    pub const FOCUSABLE: PropertyKey<bool> = PropertyKey::new(super::FIGURE_NAMESPACE, "focusable");
    /// Figure focus traversal eligibility.
    pub const FOCUS_TRAVERSABLE: PropertyKey<bool> =
        PropertyKey::new(super::FIGURE_NAMESPACE, "focus_traversable");
    /// Clickable selection state.
    pub const SELECTED: PropertyKey<bool> = PropertyKey::new(super::FIGURE_NAMESPACE, "selected");
    /// Clickable rollover behavior.
    pub const ROLLOVER_ENABLED: PropertyKey<bool> =
        PropertyKey::new(super::FIGURE_NAMESPACE, "rollover_enabled");
    /// Border presence.
    pub const BORDER: PropertyKey<bool> = PropertyKey::new(super::FIGURE_NAMESPACE, "border");
    /// Point-list geometry.
    pub const POINTS: PropertyKey<Vec<Point>> = PropertyKey::new(super::FIGURE_NAMESPACE, "points");
    /// Stroke width.
    pub const STROKE_WIDTH: PropertyKey<f64> =
        PropertyKey::new(super::FIGURE_NAMESPACE, "stroke_width");
    /// Complete stroke-style diagnostic.
    pub const STROKE_STYLE: PropertyKey<StrokeStyle> =
        PropertyKey::new(super::FIGURE_NAMESPACE, "stroke_style");
    /// Stroke line-join diagnostic.
    pub const LINE_JOIN: PropertyKey<LineJoin> =
        PropertyKey::new(super::FIGURE_NAMESPACE, "line_join");
    /// Scalable polygon template.
    pub const POLYGON_TEMPLATE: PropertyKey<Vec<Point>> =
        PropertyKey::new(super::FIGURE_NAMESPACE, "polygon_template");
    /// Scalable polygon mode diagnostic.
    pub const POLYGON_SCALE_MODE: PropertyKey<PolygonScaleMode> =
        PropertyKey::new(super::FIGURE_NAMESPACE, "polygon_scale_mode");
    /// Scalable polygon alignment diagnostic.
    pub const POLYGON_ALIGNMENT: PropertyKey<(Alignment, Alignment)> =
        PropertyKey::new(super::FIGURE_NAMESPACE, "polygon_alignment");
    /// Text-flow page diagnostic.
    pub const FLOW_PAGE: PropertyKey<FlowPage> =
        PropertyKey::new(super::FIGURE_NAMESPACE, "flow_page");
    /// Text-flow wrapping diagnostic.
    pub const FLOW_WRAPPING: PropertyKey<FlowWrapping> =
        PropertyKey::new(super::FIGURE_NAMESPACE, "flow_wrapping");
    /// Rounded rectangle corner dimensions.
    pub const CORNER_DIMENSIONS: PropertyKey<Dimension> =
        PropertyKey::new(super::FIGURE_NAMESPACE, "corner_dimensions");
    /// Triangle direction diagnostic.
    pub const DIRECTION: PropertyKey<Direction> =
        PropertyKey::new(super::FIGURE_NAMESPACE, "direction");
    /// Image resource diagnostic.
    pub const IMAGE: PropertyKey<ImageId> = PropertyKey::new(super::FIGURE_NAMESPACE, "image");
    /// Image alignment diagnostic.
    pub const IMAGE_ALIGNMENT: PropertyKey<Alignment> =
        PropertyKey::new(super::FIGURE_NAMESPACE, "image_alignment");
    /// Label text.
    pub const TEXT: PropertyKey<String> = PropertyKey::new(super::FIGURE_NAMESPACE, "text");
    /// Label icon diagnostic.
    pub const ICON: PropertyKey<Option<ImageId>> =
        PropertyKey::new(super::FIGURE_NAMESPACE, "icon");
    /// Label text-placement diagnostic.
    pub const TEXT_PLACEMENT: PropertyKey<TextPlacement> =
        PropertyKey::new(super::FIGURE_NAMESPACE, "text_placement");
    /// Label box alignment diagnostic.
    pub const LABEL_ALIGNMENT: PropertyKey<Alignment> =
        PropertyKey::new(super::FIGURE_NAMESPACE, "label_alignment");
    /// Label text alignment diagnostic.
    pub const TEXT_ALIGNMENT: PropertyKey<Alignment> =
        PropertyKey::new(super::FIGURE_NAMESPACE, "text_alignment");
    /// Label icon alignment diagnostic.
    pub const ICON_ALIGNMENT: PropertyKey<Alignment> =
        PropertyKey::new(super::FIGURE_NAMESPACE, "icon_alignment");
    /// Label icon/text gap.
    pub const ICON_TEXT_GAP: PropertyKey<f64> =
        PropertyKey::new(super::FIGURE_NAMESPACE, "icon_text_gap");
    /// Freeform content extent.
    pub const FREEFORM_EXTENT: PropertyKey<Rectangle> =
        PropertyKey::new(super::CONTAINER_NAMESPACE, "freeform_extent");
    /// Viewport logical location.
    pub const VIEW_LOCATION: PropertyKey<Point> =
        PropertyKey::new(super::CONTAINER_NAMESPACE, "viewLocation");
    /// Horizontal viewport location.
    pub const HORIZONTAL_VIEW_LOCATION: PropertyKey<f64> =
        PropertyKey::new(super::CONTAINER_NAMESPACE, "horizontalViewLocation");
    /// Vertical viewport location.
    pub const VERTICAL_VIEW_LOCATION: PropertyKey<f64> =
        PropertyKey::new(super::CONTAINER_NAMESPACE, "verticalViewLocation");
    /// Content scale.
    pub const SCALE: PropertyKey<f64> = PropertyKey::new(super::CONTAINER_NAMESPACE, "scale");
    /// Foreground style.
    pub const FOREGROUND: PropertyKey<Option<Color>> =
        PropertyKey::new(super::STYLE_NAMESPACE, "foreground");
    /// Background style.
    pub const BACKGROUND: PropertyKey<Option<Color>> =
        PropertyKey::new(super::STYLE_NAMESPACE, "background");
    /// Alpha style.
    pub const ALPHA: PropertyKey<Option<f64>> = PropertyKey::new(super::STYLE_NAMESPACE, "alpha");
    /// Font descriptor style.
    pub const FONT: PropertyKey<Option<String>> = PropertyKey::new(super::STYLE_NAMESPACE, "font");
    /// Cursor style.
    pub const CURSOR: PropertyKey<Option<CursorIcon>> =
        PropertyKey::new(super::STYLE_NAMESPACE, "cursor");
    /// Tooltip style.
    pub const TOOLTIP: PropertyKey<Option<String>> =
        PropertyKey::new(super::STYLE_NAMESPACE, "tooltip");
}
