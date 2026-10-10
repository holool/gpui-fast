//! Geometry follows shaped clusters, never reversed UTF-8 or glyph order.
use crate::{LineLayout, Pixels, px};
use std::{ops::Range, sync::Arc};
use unicode_bidi::BidiInfo;
use unicode_segmentation::UnicodeSegmentation;

/// Carets and selectable intervals for a bidirectional shaped line.
#[derive(Debug, Default)]
pub struct BidiGeometry {
    carets: Vec<(usize, Pixels)>,
    spans: Vec<(Range<usize>, Range<Pixels>)>,
}
impl BidiGeometry {
    /// Construct geometry from a native shaper's primary and secondary caret positions.
    /// The callback receives a UTF-8 boundary; all published carets stay on graphemes.
    pub fn from_native(
        text: &str,
        mut caret: impl FnMut(usize) -> (Pixels, Pixels),
    ) -> Option<Arc<Self>> {
        if !has_rtl(text) {
            return None;
        }
        let bidi = BidiInfo::new(text, None);
        if !bidi.levels.iter().any(|l| l.is_rtl()) {
            return None;
        }
        let boundaries: Vec<_> = text
            .grapheme_indices(true)
            .map(|(i, _)| i)
            .chain([text.len()])
            .collect();
        let mut result = Self::default();
        for pair in boundaries.windows(2) {
            let (a, b) = caret(pair[0]);
            let (c, d) = caret(pair[1]);
            let rtl = bidi.levels[pair[0]].is_rtl();
            let candidates = [(a, c), (a, d), (b, c), (b, d)];
            let (leading, trailing) = candidates
                .into_iter()
                .filter(|(a, b)| if rtl { a > b } else { a < b })
                .min_by(|(a, b), (c, d)| (*a - *b).abs().partial_cmp(&(*c - *d).abs()).unwrap())
                .unwrap_or((a, c));
            result.carets.push((pair[0], leading));
            if pair[1] == text.len() {
                result.carets.push((pair[1], trailing));
            }
            result.spans.push((
                pair[0]..pair[1],
                leading.min(trailing)..leading.max(trailing),
            ));
        }
        Some(Arc::new(result))
    }
    fn slice(&self, range: Range<usize>) -> Arc<Self> {
        Arc::new(Self {
            carets: self
                .carets
                .iter()
                .filter(|(i, _)| range.contains(i) || *i == range.end)
                .map(|(i, x)| (*i - range.start, *x))
                .collect(),
            spans: self
                .spans
                .iter()
                .filter(|(r, _)| r.start >= range.start && r.end <= range.end)
                .map(|(r, x)| ((r.start - range.start)..(r.end - range.start), x.clone()))
                .collect(),
        })
    }
    /// Construct geometry from native cluster advances; zero-width marks share a grapheme.
    pub fn from_clusters(
        text: &str,
        clusters: impl IntoIterator<Item = (Range<usize>, Pixels, Pixels, bool)>,
    ) -> Option<Arc<Self>> {
        if !has_rtl(text) {
            return None;
        }
        let mut result = Self::default();
        for (range, left, right, rtl) in clusters {
            let boundaries: Vec<_> = text
                .grapheme_indices(true)
                .map(|(i, _)| i)
                .filter(|i| range.contains(i))
                .chain([range.end])
                .collect();
            let count = boundaries.len().saturating_sub(1);
            if count == 0 {
                continue;
            }
            for (n, pair) in boundaries.windows(2).enumerate() {
                let a = (right - left) * (n as f32 / count as f32);
                let b = (right - left) * ((n + 1) as f32 / count as f32);
                let (leading, trailing) = if rtl {
                    (right - a, right - b)
                } else {
                    (left + a, left + b)
                };
                result.carets.push((pair[0], leading));
                if pair[1] == text.len() {
                    result.carets.push((pair[1], trailing));
                }
                result.spans.push((
                    pair[0]..pair[1],
                    leading.min(trailing)..leading.max(trailing),
                ));
            }
        }
        result.carets.sort_by_key(|c| c.0);
        result.carets.dedup_by_key(|c| c.0);
        (!result.carets.is_empty()).then(|| Arc::new(result))
    }
    pub(crate) fn build(text: &str, layout: &LineLayout) -> Option<Arc<Self>> {
        Self::build_with_direction(text, layout, None)
    }
    pub(crate) fn build_with_direction(
        text: &str,
        layout: &LineLayout,
        direction: Option<crate::LayoutDirection>,
    ) -> Option<Arc<Self>> {
        if direction != Some(crate::LayoutDirection::Rtl) && !has_rtl(text) {
            return None;
        }
        let bidi = BidiInfo::new(
            text,
            direction.map(|d| {
                if d.is_rtl() {
                    unicode_bidi::Level::rtl()
                } else {
                    unicode_bidi::Level::ltr()
                }
            }),
        );
        if !bidi.levels.iter().any(|level| level.is_rtl()) {
            return None;
        }
        let boundaries: Vec<usize> = text
            .grapheme_indices(true)
            .map(|(i, _)| i)
            .chain([text.len()])
            .collect();
        let mut clusters: Vec<(usize, Pixels)> = layout
            .runs
            .iter()
            .flat_map(|run| &run.glyphs)
            .filter(|g| g.index < text.len())
            .map(|g| {
                let i = boundaries
                    .partition_point(|i| *i <= g.index)
                    .saturating_sub(1);
                (boundaries[i], g.position.x)
            })
            .collect();
        clusters.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.partial_cmp(&b.1).unwrap()));
        clusters.dedup_by_key(|c| c.0);
        let mut result = Self::default();
        for (n, &(start, left)) in clusters.iter().enumerate() {
            let end = clusters.get(n + 1).map_or(text.len(), |c| c.0);
            let right = clusters
                .iter()
                .filter(|c| c.1 > left)
                .map(|c| c.1)
                .min_by(|a, b| a.partial_cmp(b).unwrap())
                .unwrap_or(layout.width);
            let graphemes: Vec<usize> = boundaries
                .iter()
                .copied()
                .filter(|i| *i >= start && *i < end)
                .chain([end])
                .collect();
            let count = graphemes.len().saturating_sub(1);
            if count == 0 {
                continue;
            }
            let rtl = bidi.levels.get(start).is_some_and(|l| l.is_rtl());
            for (i, range) in graphemes.windows(2).enumerate() {
                let a = left + (right - left) * (i as f32 / count as f32);
                let b = left + (right - left) * ((i + 1) as f32 / count as f32);
                let (leading, trailing) = if rtl {
                    (right - (a - left), right - (b - left))
                } else {
                    (a, b)
                };
                result.carets.push((range[0], leading));
                if range[1] == text.len() {
                    result.carets.push((range[1], trailing));
                }
                result.spans.push((
                    range[0]..range[1],
                    leading.min(trailing)..leading.max(trailing),
                ));
            }
        }
        if result.carets.is_empty() {
            return None;
        }
        result.carets.sort_by_key(|c| c.0);
        result.carets.dedup_by_key(|c| c.0);
        Some(Arc::new(result))
    }
    pub(crate) fn x(&self, index: usize) -> Pixels {
        self.carets
            .iter()
            .find(|c| c.0 >= index)
            .or_else(|| self.carets.last())
            .map_or(px(0.), |c| c.1)
    }
    pub(crate) fn closest(&self, x: Pixels) -> usize {
        self.carets
            .iter()
            .min_by(|a, b| (a.1 - x).abs().partial_cmp(&(b.1 - x).abs()).unwrap())
            .map_or(0, |c| c.0)
    }
    pub(crate) fn step(&self, index: usize, right: bool) -> usize {
        let x = self.x(index);
        self.carets
            .iter()
            .filter(|c| if right { c.1 > x } else { c.1 < x })
            .min_by(|a, b| (a.1 - x).abs().partial_cmp(&(b.1 - x).abs()).unwrap())
            .map_or(index, |c| c.0)
    }
    pub(crate) fn ranges(&self, range: Range<usize>) -> Vec<Range<Pixels>> {
        let mut spans: Vec<_> = self
            .spans
            .iter()
            .filter(|(r, _)| r.start < range.end && r.end > range.start)
            .map(|(_, x)| x.clone())
            .collect();
        spans.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap());
        let mut merged: Vec<Range<Pixels>> = Vec::new();
        for span in spans {
            if let Some(last) = merged.last_mut() {
                if span.start <= last.end + px(0.01) {
                    last.end = last.end.max(span.end);
                    continue;
                }
            }
            merged.push(span);
        }
        merged
    }
}
impl LineLayout {
    /// Physical intervals occupied by a logical UTF-8 selection (possibly disjoint).
    pub fn selection_ranges(&self, range: Range<usize>) -> Vec<Range<Pixels>> {
        if range.is_empty() {
            return Vec::new();
        }
        if let Some(geometry) = &self.fast_bidi {
            return geometry.ranges(range);
        }
        vec![self.x_for_index(range.start)..self.x_for_index(range.end)]
    }
    /// Next grapheme caret in visual order. Returns the original index at an edge.
    pub fn visual_caret(&self, index: usize, right: bool) -> usize {
        self.fast_bidi
            .as_ref()
            .map_or(index, |g| g.step(index, right))
    }
}

impl crate::WindowTextSystem {
    /// Shape with an explicit paragraph base direction, preserving source byte offsets.
    pub fn shape_line_in_direction(
        &self,
        text: crate::SharedString,
        font_size: Pixels,
        runs: &[crate::TextRun],
        force_width: Option<Pixels>,
        direction: crate::LayoutDirection,
    ) -> crate::ShapedLine {
        if text.is_empty() || (!direction.is_rtl() && !has_rtl(&text)) {
            return self.shape_line(text, font_size, runs, force_width);
        }
        let marker = if direction.is_rtl() {
            '\u{200f}'
        } else {
            '\u{200e}'
        };
        let mut marked_runs = runs.to_vec();
        if let Some(first) = marked_runs.first_mut() {
            first.len += marker.len_utf8();
        }
        let mut line = self.shape_line(
            format!("{marker}{text}").into(),
            font_size,
            &marked_runs,
            force_width,
        );
        let old = &*line.layout;
        let mut glyph_runs = old.runs.clone();
        for run in &mut glyph_runs {
            run.glyphs.retain(|g| g.index >= marker.len_utf8());
            for g in &mut run.glyphs {
                g.index -= marker.len_utf8();
            }
        }
        let mut layout = LineLayout {
            fast_bidi: None,
            font_size: old.font_size,
            width: old.width,
            ascent: old.ascent,
            descent: old.descent,
            runs: glyph_runs,
            len: text.len(),
        };
        layout.fast_bidi = old
            .fast_bidi
            .as_ref()
            .map(|g| g.slice(marker.len_utf8()..old.len))
            .or_else(|| BidiGeometry::build_with_direction(&text, &layout, Some(direction)));
        line.layout = Arc::new(layout);
        line.text = text;
        if let Some(first) = line.decoration_runs.first_mut() {
            first.len -= marker.len_utf8() as u32;
        }
        line
    }
    /// Shape wrapped text by logical line boundaries, then shape each visual row independently.
    pub fn shape_text_in_direction(
        &self,
        text: crate::SharedString,
        font_size: Pixels,
        runs: &[crate::TextRun],
        wrap_width: Option<Pixels>,
        line_clamp: Option<usize>,
        direction: crate::LayoutDirection,
    ) -> crate::Result<smallvec::SmallVec<[crate::WrappedLine; 1]>> {
        let mut result = smallvec::SmallVec::new();
        let mut offset = 0;
        for paragraph in text.split('\n') {
            let mut start = 0;
            loop {
                if line_clamp.is_some_and(|limit| result.len() >= limit) {
                    return Ok(result);
                }
                let mut end = paragraph.len();
                if let Some(width) = wrap_width {
                    let boundaries: Vec<_> = paragraph[start..]
                        .grapheme_indices(true)
                        .map(|(i, g)| start + i + g.len())
                        .collect();
                    let fits = |n: usize| {
                        let candidate = boundaries[n - 1];
                        let line_runs = slice_runs(runs, offset + start..offset + candidate);
                        self.shape_line_in_direction(
                            paragraph[start..candidate].to_string().into(),
                            font_size,
                            &line_runs,
                            None,
                            direction,
                        )
                        .width
                            <= width
                    };
                    // Exponential search bounds each row's work by its own length.
                    let mut low = 1;
                    let mut high = 1;
                    while high < boundaries.len() && fits(high) {
                        low = high;
                        high = (high * 2).min(boundaries.len());
                    }
                    if !boundaries.is_empty() {
                        if fits(high) {
                            low = high;
                        } else {
                            while low + 1 < high {
                                let mid = (low + high) / 2;
                                if fits(mid) {
                                    low = mid;
                                } else {
                                    high = mid;
                                }
                            }
                        }
                        end = boundaries[low - 1];
                        if end < paragraph.len() {
                            if let Some((word, _)) =
                                unicode_linebreak::linebreaks(&paragraph[start..end])
                                    .filter(|(i, _)| *i < end - start)
                                    .last()
                            {
                                if word > 0 {
                                    end = start + word;
                                }
                            }
                        }
                    }
                }
                let line_runs = slice_runs(runs, offset + start..offset + end);
                let line = self.shape_line_in_direction(
                    paragraph[start..end].to_string().into(),
                    font_size,
                    &line_runs,
                    None,
                    direction,
                );
                result.push(crate::WrappedLine {
                    fast_soft_wrap: end < paragraph.len(),
                    text: line.text.clone(),
                    layout: Arc::new(crate::WrappedLineLayout {
                        unwrapped_layout: line.layout,
                        wrap_boundaries: smallvec::SmallVec::new(),
                        wrap_width,
                    }),
                    decoration_runs: line.decoration_runs.into_vec(),
                });
                if end == paragraph.len() {
                    break;
                }
                start = end;
            }
            offset += paragraph.len() + 1;
        }
        Ok(result)
    }
}
fn slice_runs(runs: &[crate::TextRun], range: Range<usize>) -> Vec<crate::TextRun> {
    let mut offset = 0;
    runs.iter()
        .filter_map(|run| {
            let start = offset;
            offset += run.len;
            let len = offset.min(range.end).saturating_sub(start.max(range.start));
            (len > 0).then(|| crate::TextRun { len, ..run.clone() })
        })
        .collect()
}
fn aligned_origin(
    origin: crate::Point<Pixels>,
    layout: &LineLayout,
    align: crate::TextAlign,
    width: Option<Pixels>,
) -> crate::Point<Pixels> {
    let extra = width.unwrap_or(layout.width) - layout.width;
    origin
        + crate::point(
            match align {
                crate::TextAlign::Left => px(0.),
                crate::TextAlign::Center => extra / 2.,
                crate::TextAlign::Right => extra,
            },
            px(0.),
        )
}
pub(crate) fn paint(
    layout: &LineLayout,
    geometry: (
        crate::Point<Pixels>,
        Pixels,
        crate::TextAlign,
        Option<Pixels>,
    ),
    decoration: (
        &[crate::DecorationRun],
        &mut crate::Window,
        &mut dyn FnMut(
            Range<usize>,
            crate::Point<Pixels>,
            Pixels,
            &crate::UnderlineStyle,
            &mut crate::Window,
        ),
    ),
) -> crate::Result<()> {
    let (origin, line_height, align, width) = geometry;
    let (decorations, window, paint_underline) = decoration;
    let origin = aligned_origin(origin, layout, align, width);
    let baseline = (line_height - layout.ascent - layout.descent) / 2. + layout.ascent;
    let mut offset = 0;
    for decoration in decorations {
        let range = offset..offset + decoration.len as usize;
        offset = range.end;
        for span in layout.selection_ranges(range.clone()) {
            if let Some(mut underline) = decoration.underline {
                underline.color = Some(underline.color.unwrap_or(decoration.color));
                paint_underline(
                    range.clone(),
                    origin
                        + crate::point(
                            span.start,
                            crate::underline_y_offset(line_height, layout.ascent, layout.descent),
                        ),
                    span.end - span.start,
                    &underline,
                    window,
                );
            }
            if let Some(mut strike) = decoration.strikethrough {
                strike.color = Some(strike.color.unwrap_or(decoration.color));
                window.paint_strikethrough(
                    origin + crate::point(span.start, baseline - layout.ascent / 3.),
                    span.end - span.start,
                    &strike,
                );
            }
        }
    }
    for run in &layout.runs {
        for glyph in &run.glyphs {
            let mut end = 0;
            let color = decorations
                .iter()
                .find(|d| {
                    end += d.len as usize;
                    glyph.index < end
                })
                .map_or(crate::black(), |d| d.color);
            let position = origin + glyph.position + crate::point(px(0.), baseline);
            if glyph.is_emoji {
                window.paint_emoji(position, run.font_id, glyph.id, layout.font_size)?;
            } else {
                window.paint_glyph(position, run.font_id, glyph.id, layout.font_size, color)?;
            }
        }
    }
    Ok(())
}
pub(crate) fn backgrounds(
    layout: &LineLayout,
    geometry: (
        crate::Point<Pixels>,
        Pixels,
        crate::TextAlign,
        Option<Pixels>,
    ),
    decoration: (&[crate::DecorationRun], &mut crate::Window),
) -> crate::Result<()> {
    let (origin, line_height, align, width) = geometry;
    let (decorations, window) = decoration;
    let origin = aligned_origin(origin, layout, align, width);
    let mut offset = 0;
    for d in decorations {
        let range = offset..offset + d.len as usize;
        offset = range.end;
        if let Some(color) = d.background_color {
            for span in layout.selection_ranges(range) {
                window.paint_quad(crate::fill(
                    crate::Bounds::new(
                        origin + crate::point(span.start, px(0.)),
                        crate::size(span.end - span.start, line_height),
                    ),
                    color,
                ));
            }
        }
    }
    Ok(())
}

pub(crate) fn has_geometry(layout: &LineLayout) -> bool {
    layout.fast_bidi.is_some()
}

pub(crate) fn split(layout: &LineLayout, index: usize) -> (LineLayout, LineLayout) {
    fn part(layout: &LineLayout, range: Range<usize>) -> LineLayout {
        let spans = layout.selection_ranges(range.clone());
        let left = spans.first().map_or(px(0.), |s| s.start);
        let right = spans.last().map_or(left, |s| s.end);
        let mut runs = layout.runs.clone();
        for run in &mut runs {
            run.glyphs.retain(|g| range.contains(&g.index));
            for glyph in &mut run.glyphs {
                glyph.index -= range.start;
                glyph.position.x -= left;
            }
        }
        runs.retain(|r| !r.glyphs.is_empty());
        let geometry = layout.fast_bidi.as_ref().map(|g| {
            let mut geometry = g.slice(range.clone());
            let slice = Arc::get_mut(&mut geometry).unwrap();
            for (_, x) in &mut slice.carets {
                *x -= left;
            }
            for (_, x) in &mut slice.spans {
                x.start -= left;
                x.end -= left;
            }
            geometry
        });
        LineLayout {
            fast_bidi: geometry,
            font_size: layout.font_size,
            width: right - left,
            ascent: layout.ascent,
            descent: layout.descent,
            runs,
            len: range.len(),
        }
    }
    let index = index.min(layout.len);
    (part(layout, 0..index), part(layout, index..layout.len))
}

pub(crate) fn line_advance(line: &crate::WrappedLine) -> usize {
    line.len() + usize::from(!line.fast_soft_wrap)
}
pub(crate) fn has_rtl(text: &str) -> bool {
    !text.is_ascii() && BidiInfo::new(text, None).levels.iter().any(|l| l.is_rtl())
}
pub(crate) fn shape_text_auto(
    system: &crate::WindowTextSystem,
    text: crate::SharedString,
    font_size: Pixels,
    runs: &[crate::TextRun],
    width: Option<Pixels>,
    clamp: Option<usize>,
) -> crate::Result<smallvec::SmallVec<[crate::WrappedLine; 1]>> {
    let mut result = smallvec::SmallVec::new();
    let mut offset = 0;
    for paragraph in text.split('\n') {
        if clamp.is_some_and(|limit| result.len() >= limit) {
            break;
        }
        let direction = if BidiInfo::new(paragraph, None)
            .paragraphs
            .first()
            .is_some_and(|p| p.level.is_rtl())
        {
            crate::LayoutDirection::Rtl
        } else {
            crate::LayoutDirection::Ltr
        };
        result.extend(system.shape_text_in_direction(
            paragraph.to_string().into(),
            font_size,
            &slice_runs(runs, offset..offset + paragraph.len()),
            width,
            clamp.map(|c| c - result.len()),
            direction,
        )?);
        offset += paragraph.len() + 1;
    }
    Ok(result)
}
