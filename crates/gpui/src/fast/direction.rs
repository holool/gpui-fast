//! Inherited inline direction, shared by layout, text and retained contexts.
use crate::{DefiniteLength, Length, Refineable, Style, Styled, TextAlign, Window};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The inline progression direction. Unspecified styles inherit their parent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub enum LayoutDirection {
    /// Inline start is the left edge.
    #[default]
    Ltr,
    /// Inline start is the right edge.
    Rtl,
}
impl LayoutDirection {
    /// Whether inline start is on the right.
    pub fn is_rtl(self) -> bool {
        self == Self::Rtl
    }
    /// Physical alignment at inline start.
    pub fn text_align(self) -> TextAlign {
        if self.is_rtl() {
            TextAlign::Right
        } else {
            TextAlign::Left
        }
    }
}
/// Direction builders for any styled element. Physical edges remain physical.
pub trait Directional: Styled {
    /// Set a direction boundary, inherited by descendants. Defaults text to inline start.
    fn direction(mut self, direction: LayoutDirection) -> Self {
        let text = &mut self.style().text;
        text.fast_direction = Some(direction);
        if text.text_align.is_none() {
            text.text_align = Some(direction.text_align());
        }
        self
    }
    /// Set padding start in inherited inline direction.
    fn ps(mut self, value: impl Into<DefiniteLength>) -> Self {
        self.style().fast_logical.padding_start = Some(value.into());
        self
    }
    /// Set padding end in inherited inline direction.
    fn pe(mut self, value: impl Into<DefiniteLength>) -> Self {
        self.style().fast_logical.padding_end = Some(value.into());
        self
    }
    /// Set margin start in inherited inline direction.
    fn ms(mut self, value: impl Into<Length>) -> Self {
        self.style().fast_logical.margin_start = Some(value.into());
        self
    }
    /// Set margin end in inherited inline direction.
    fn me(mut self, value: impl Into<Length>) -> Self {
        self.style().fast_logical.margin_end = Some(value.into());
        self
    }
    /// Set inset start in inherited inline direction.
    fn start(mut self, value: impl Into<Length>) -> Self {
        self.style().fast_logical.inset_start = Some(value.into());
        self
    }
    /// Set inset end in inherited inline direction.
    fn end(mut self, value: impl Into<Length>) -> Self {
        self.style().fast_logical.inset_end = Some(value.into());
        self
    }
    /// Set a right-to-left boundary.
    fn rtl(self) -> Self {
        self.direction(LayoutDirection::Rtl)
    }
    /// Set a left-to-right boundary, for example around source code.
    fn ltr(self) -> Self {
        self.direction(LayoutDirection::Ltr)
    }
}
impl<T: Styled> Directional for T {}
impl Window {
    /// Direction of the current element's inherited text/style context.
    pub fn layout_direction(&self) -> LayoutDirection {
        self.text_style().fast_direction
    }
}
pub(crate) fn inherit_style(mut style: Style, window: &Window) -> Style {
    style
        .text
        .fast_direction
        .get_or_insert(window.layout_direction());
    resolve_logical(&mut style);
    style
}
pub(crate) fn taffy_direction(style: &Style) -> taffy::style::Direction {
    if style.text.fast_direction.unwrap_or_default().is_rtl() {
        taffy::style::Direction::Rtl
    } else {
        taffy::style::Direction::Ltr
    }
}

/// Logical horizontal edges. Unset edges keep their physical style values.
#[derive(Clone, Debug, Default, Refineable)]
#[refineable(Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct LogicalStyle {
    /// Padding at inline start.
    pub padding_start: Option<DefiniteLength>,
    /// Padding at inline end.
    pub padding_end: Option<DefiniteLength>,
    /// Margin at inline start.
    pub margin_start: Option<Length>,
    /// Margin at inline end.
    pub margin_end: Option<Length>,
    /// Positioned inset at inline start.
    pub inset_start: Option<Length>,
    /// Positioned inset at inline end.
    pub inset_end: Option<Length>,
}
fn resolve_logical(style: &mut Style) {
    let rtl = style.text.fast_direction.unwrap_or_default().is_rtl();
    let logical = &style.fast_logical;
    let (ps, pe, ms, me, start, end) = if rtl {
        (
            logical.padding_end,
            logical.padding_start,
            logical.margin_end,
            logical.margin_start,
            logical.inset_end,
            logical.inset_start,
        )
    } else {
        (
            logical.padding_start,
            logical.padding_end,
            logical.margin_start,
            logical.margin_end,
            logical.inset_start,
            logical.inset_end,
        )
    };
    if let Some(v) = ps {
        style.padding.left = v;
    }
    if let Some(v) = pe {
        style.padding.right = v;
    }
    if let Some(v) = ms {
        style.margin.left = v;
    }
    if let Some(v) = me {
        style.margin.right = v;
    }
    if let Some(v) = start {
        style.inset.left = v;
    }
    if let Some(v) = end {
        style.inset.right = v;
    }
}

pub(crate) fn needs_resolution(style: &Style, window: &Window) -> bool {
    let logical = &style.fast_logical;
    (style.text.fast_direction.is_none() && window.layout_direction().is_rtl())
        || logical.padding_start.is_some()
        || logical.padding_end.is_some()
        || logical.margin_start.is_some()
        || logical.margin_end.is_some()
        || logical.inset_start.is_some()
        || logical.inset_end.is_some()
}
