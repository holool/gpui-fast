//! DirectWrite's native direction and cluster map define visual positions.
use gpui::{BidiGeometry, Pixels, px};
use std::{ops::Range, sync::Arc};
use windows::Win32::Graphics::DirectWrite::{DWRITE_HIT_TEST_METRICS, IDWriteTextLayout};
pub(crate) fn clusters(map: &[u16], count: usize) -> Vec<(Range<usize>, Range<usize>)> {
    let mut starts = map.iter().map(|i| *i as usize).collect::<Vec<_>>();
    starts.sort_unstable();
    starts.dedup();
    starts.push(count);
    let mut result = Vec::new();
    let mut start = 0;
    while start < map.len() {
        let glyph = map[start] as usize;
        let mut end = start + 1;
        while end < map.len() && map[end] == map[start] {
            end += 1;
        }
        let next = starts.iter().copied().find(|i| *i > glyph).unwrap_or(count);
        result.push((start..end, glyph..next));
        start = end;
    }
    result
}
pub(crate) fn glyph_x(baseline: f32, advance: f32, width: f32, offset: f32, level: u32) -> Pixels {
    px(if level % 2 == 1 {
        baseline - advance - width - offset
    } else {
        baseline + advance + offset
    })
}
pub(crate) fn bidi_geometry(text: &str, layout: &IDWriteTextLayout) -> Option<Arc<BidiGeometry>> {
    BidiGeometry::from_native(text, |index| {
        let utf16 = text[..index].encode_utf16().count() as u32;
        let hit = |position, trailing| {
            let mut x = 0.;
            let mut y = 0.;
            let mut metrics = DWRITE_HIT_TEST_METRICS::default();
            let _ = unsafe {
                layout.HitTestTextPosition(position, trailing, &mut x, &mut y, &mut metrics)
            };
            px(x)
        };
        (
            hit(utf16, false),
            if utf16 > 0 {
                hit(utf16 - 1, true)
            } else {
                hit(0, false)
            },
        )
    })
}
#[cfg(test)]
mod tests {
    use super::{clusters, glyph_x};
    use gpui::px;
    #[test]
    fn reversed_clusters_keep_source_indices_and_advance_left() {
        assert_eq!(
            clusters(&[2, 1, 1, 0], 3),
            vec![(0..1, 2..3), (1..3, 1..2), (3..4, 0..1)]
        );
        assert_eq!(glyph_x(30., 0., 10., 0., 1), px(20.));
        assert_eq!(glyph_x(30., 10., 10., 0., 1), px(10.));
        assert_eq!(glyph_x(0., 10., 10., 0., 0), px(10.));
    }
}

pub(crate) fn reset_converter(converter: &mut crate::direct_write::StringIndexConverter<'_>) {
    converter.utf8_ix = 0;
    converter.utf16_ix = 0;
}

pub(crate) fn glyph_positions(
    baseline: f32,
    advances: &[f32],
    offsets: &[windows::Win32::Graphics::DirectWrite::DWRITE_GLYPH_OFFSET],
    level: u32,
) -> Vec<Pixels> {
    let mut advance = 0.;
    advances
        .iter()
        .zip(offsets)
        .map(|(width, offset)| {
            let x = glyph_x(baseline, advance, *width, offset.advanceOffset, level);
            advance += width;
            x
        })
        .collect()
}
