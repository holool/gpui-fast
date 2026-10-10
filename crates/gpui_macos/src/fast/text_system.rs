//! Native fonts made once per size and kept, so CoreText's shaping caches survive from line to line.

use collections::HashMap;
use core_text::font::CTFont;
use font_kit::font::Font as FontKitFont;
use gpui::{FontId, Pixels};

/// Each font at each size a line has been laid out at. CoreText keeps the
/// caches it builds for shaping — the advances and glyphs of ASCII among
/// them — on the font object, so a font made afresh for every line built
/// them again for every line.
#[derive(Default)]
pub(crate) struct SizedFonts(HashMap<(FontId, u32), CTFont>);

impl SizedFonts {
    /// The native font for `font`, whose id is `font_id`, at `font_size`, made
    /// once and kept.
    pub(crate) fn get(&mut self, font_id: FontId, font: &FontKitFont, font_size: Pixels) -> CTFont {
        // Sizes can animate, and every size a font is drawn at would otherwise
        // be kept for good.
        const MAX_SIZED_FONTS: usize = 1024;
        let key = (font_id, f32::from(font_size).to_bits());
        if let Some(font) = self.0.get(&key) {
            return font.clone();
        }
        if self.0.len() >= MAX_SIZED_FONTS {
            self.0.clear();
        }
        let font = font
            .native_font()
            .clone_with_font_size(f32::from(font_size).into());
        self.0.insert(key, font.clone());
        font
    }
}

#[cfg(test)]
mod tests {
    use crate::MacTextSystem;
    use gpui::{FontRun, PlatformTextSystem, font, px};

    /// Fonts are made once per size and kept, so a line laid out with a kept
    /// font has to come out exactly as it did with a new one: the same glyphs
    /// in the same places, however many runs split it into sizes a hair
    /// apart, and never mixed up with the same font at another size.
    #[test]
    fn lines_laid_out_with_kept_fonts_match_the_first_layout() {
        let fonts = MacTextSystem::new();
        let font_id = fonts.font_id(&font("Helvetica")).unwrap();
        let line = "AVAWAY fi ffi 12.5 office";
        // Runs alternate between two sizes a hair apart, to keep ligatures
        // from joining across them.
        let runs = [5, 3, 4, 5, 8].map(|len| FontRun { font_id, len });
        let shape = |size| {
            let layout = fonts.layout_line(line, px(size), &runs);
            let glyphs = layout
                .runs
                .iter()
                .flat_map(|run| run.glyphs.iter().map(|glyph| (glyph.id, glyph.position)))
                .collect::<Vec<_>>();
            (layout.width, glyphs)
        };

        let first = shape(16.);
        assert_eq!(shape(16.), first, "a kept font shapes differently");
        let larger = shape(24.);
        assert_ne!(larger.0, first.0, "another size has to be another font");
        assert_eq!(shape(16.), first, "a size laid out again after another");
        assert_eq!(shape(24.), larger);
    }
}

#[cfg(test)]
mod rtl_tests {
    use crate::MacTextSystem;
    use gpui::{LayoutDirection, TextRun, TextSystem, WindowTextSystem, black, font, px};
    use std::sync::Arc;
    #[test]
    fn native_arabic_caret_geometry_and_wrapping_preserve_utf8() {
        let system =
            WindowTextSystem::new(Arc::new(TextSystem::new(Arc::new(MacTextSystem::new()))));
        for text in ["مرحبا", "أَهْلاً", "لا", "مرحبا بالعالم"] {
            let runs = [TextRun {
                len: text.len(),
                font: font("Arial"),
                color: black(),
                background_color: None,
                underline: None,
                strikethrough: None,
            }];
            let line = system.shape_line_in_direction(
                text.into(),
                px(20.),
                &runs,
                None,
                LayoutDirection::Rtl,
            );
            assert!(
                (line.x_for_index(0) - line.width).abs() < px(0.1),
                "start: {text}"
            );
            assert!(line.x_for_index(text.len()).abs() < px(0.1), "end: {text}");
            assert_eq!(line.closest_index_for_x(line.width + px(20.)), 0);
            assert_eq!(line.closest_index_for_x(px(-20.)), text.len());
            let wrapped = system
                .shape_text_in_direction(
                    text.into(),
                    px(20.),
                    &runs,
                    Some(px(45.)),
                    None,
                    LayoutDirection::Rtl,
                )
                .unwrap();
            assert_eq!(
                wrapped.iter().map(|l| l.text.as_str()).collect::<String>(),
                text
            );
            assert!(wrapped.iter().all(|l| l.len() == l.text.len()));
        }
        let text = "English (عربي) 123";
        let runs = [TextRun {
            len: text.len(),
            font: font("Arial"),
            color: black(),
            background_color: None,
            underline: None,
            strikethrough: None,
        }];
        let ltr =
            system.shape_line_in_direction(text.into(), px(20.), &runs, None, LayoutDirection::Ltr);
        let rtl =
            system.shape_line_in_direction(text.into(), px(20.), &runs, None, LayoutDirection::Rtl);
        assert_ne!(ltr.x_for_index(0), rtl.x_for_index(0));
        assert_eq!(ltr.len, text.len());
        assert_eq!(rtl.len, text.len());
    }
}

/// CoreText knows ligature carets and mark advances; use those instead of glyph ink origins.
pub(crate) fn bidi_geometry(
    text: &str,
    line: &core_text::line::CTLine,
) -> Option<std::sync::Arc<gpui::BidiGeometry>> {
    use core_foundation::base::TCFType;
    unsafe extern "C" {
        fn CTLineGetOffsetForStringIndex(
            line: core_text::line::CTLineRef,
            index: isize,
            secondary: *mut f64,
        ) -> f64;
    }
    gpui::BidiGeometry::from_native(text, |index| {
        let utf16 = text[..index].encode_utf16().count();
        let mut secondary = 0.0;
        let primary = unsafe {
            CTLineGetOffsetForStringIndex(
                line.as_concrete_TypeRef(),
                utf16 as isize,
                &mut secondary,
            )
        };
        (gpui::px(primary as f32), gpui::px(secondary as f32))
    })
}

/// Native shaping for regression harnesses without creating an AppKit application.
#[cfg(feature = "test-support")]
pub fn text_system_for_tests() -> std::sync::Arc<dyn gpui::PlatformTextSystem> {
    std::sync::Arc::new(crate::MacTextSystem::new())
}
