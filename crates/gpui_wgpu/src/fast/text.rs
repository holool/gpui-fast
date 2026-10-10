//! Native cosmic-text cluster advances, independent of combining-mark ink offsets.
use gpui::{BidiGeometry, LineLayout, px};
use std::{collections::BTreeMap, sync::Arc};
pub(crate) fn bidi_geometry(
    text: &str,
    glyphs: &[cosmic_text::LayoutGlyph],
) -> Option<Arc<BidiGeometry>> {
    let mut clusters = BTreeMap::<(usize, usize), (f32, f32, bool)>::new();
    for glyph in glyphs {
        if glyph.w <= 0.0 {
            continue;
        }
        clusters
            .entry((glyph.start, glyph.end))
            .and_modify(|c| {
                c.0 = c.0.min(glyph.x);
                c.1 = c.1.max(glyph.x + glyph.w);
            })
            .or_insert((glyph.x, glyph.x + glyph.w, glyph.level.is_rtl()));
    }
    BidiGeometry::from_clusters(
        text,
        clusters
            .into_iter()
            .map(|((start, end), (left, right, rtl))| (start..end, px(left), px(right), rtl)),
    )
}
pub(crate) fn segment_geometry(
    layout: &mut LineLayout,
    segment: &LineLayout,
    start: usize,
    end: usize,
) {
    if start == 0 && end == layout.len {
        layout.fast_bidi = segment.fast_bidi.clone();
    }
}
