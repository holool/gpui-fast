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

#[cfg(test)]
mod tests {

    /// CE's empty directional text regression, exercised through Fast's marker path.
    #[test]
    fn empty_directional_text_keeps_source_offsets_without_font_runs() {
        let system = {
            use gpui::PlatformTextSystem as _;
            let fonts = crate::CosmicTextSystem::new_without_system_fonts("IBM Plex Sans");
            fonts
                .add_fonts(vec![std::borrow::Cow::Borrowed(include_bytes!(
                    "../../../../assets/fonts/ibm-plex-sans/IBMPlexSans-Regular.ttf"
                ))])
                .unwrap();
            gpui::WindowTextSystem::new(std::sync::Arc::new(gpui::TextSystem::new(
                std::sync::Arc::new(fonts),
            )))
        };
        // Repeat across directions to exercise fresh and cached empty layouts.
        for _ in 0..2 {
            for direction in [gpui::LayoutDirection::Ltr, gpui::LayoutDirection::Rtl] {
                for runs in [
                    Vec::new(),
                    vec![gpui::TextRun {
                        len: 0,
                        font: gpui::font("IBM Plex Sans"),
                        color: gpui::black(),
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    }],
                ] {
                    let line = system.shape_line_in_direction(
                        "".into(),
                        gpui::px(20.),
                        &runs,
                        None,
                        direction,
                    );
                    assert!(line.text.is_empty());
                    assert_eq!(line.len, 0);
                    assert_eq!(line.width, gpui::px(0.));
                    assert!(line.runs.iter().all(|run| run.glyphs.is_empty()));
                    assert_eq!(line.x_for_index(0), gpui::px(0.));
                    assert_eq!(line.closest_index_for_x(gpui::px(10.)), 0);
                    for wrap_width in [None, Some(gpui::px(1.))] {
                        let lines = system
                            .shape_text_in_direction(
                                "".into(),
                                gpui::px(20.),
                                &runs,
                                wrap_width,
                                None,
                                direction,
                            )
                            .unwrap();
                        assert_eq!(lines.len(), 1);
                        assert!(lines[0].text.is_empty());
                        assert_eq!(lines[0].len(), 0);
                        assert_eq!(lines[0].unwrapped_layout.width, gpui::px(0.));
                        assert!(
                            lines[0]
                                .unwrapped_layout
                                .runs
                                .iter()
                                .all(|run| run.glyphs.is_empty())
                        );
                    }
                }
            }
        }
    }
}
