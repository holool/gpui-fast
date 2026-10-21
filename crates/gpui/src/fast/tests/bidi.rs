use crate::{
    AppContext as _, Bounds, Context, Directional as _, Entity, FontId, GlyphId, IntoElement,
    LayoutDirection, LineLayout, ParentElement as _, Pixels, Render, ShapedGlyph, ShapedRun,
    StyleRefinement, Styled as _, TestAppContext, Window, canvas, div, point, px,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

fn shaped(text: &str, visual_indices: &[usize]) -> LineLayout {
    let mut line = LineLayout {
        len: text.len(),
        width: px(visual_indices.len() as f32 * 10.),
        runs: vec![ShapedRun {
            font_id: FontId(0),
            glyphs: visual_indices
                .iter()
                .enumerate()
                .map(|(i, index)| ShapedGlyph {
                    id: GlyphId(0),
                    index: *index,
                    position: point(px(i as f32 * 10.), px(0.)),
                    is_emoji: false,
                })
                .collect(),
        }],
        ..Default::default()
    };
    line.fast_bidi = crate::fast::bidi::BidiGeometry::build(text, &line);
    line
}
#[test]
fn rtl_carets_hit_testing_and_disjoint_selections_share_shaped_geometry() {
    let line = shaped("אבג", &[4, 2, 0]);
    assert_eq!(line.x_for_index(0), px(30.));
    assert_eq!(line.x_for_index(6), px(0.));
    for (index, x) in [(0, 30.), (2, 20.), (4, 10.), (6, 0.)] {
        assert_eq!(line.closest_index_for_x(px(x)), index);
    }
    assert_eq!(line.closest_index_for_x(px(-50.)), 6);
    assert_eq!(line.closest_index_for_x(px(80.)), 0);
    assert_eq!(line.visual_caret(0, false), 2);
    assert_eq!(line.visual_caret(6, true), 4);
    assert_eq!(line.selection_ranges(0..4), vec![px(10.)..px(30.)]);
    let mixed = shaped("abc אבג def", &[0, 1, 2, 3, 8, 6, 4, 10, 11, 12, 13]);
    assert_eq!(
        mixed.selection_ranges(0..6),
        vec![px(0.)..px(40.), px(60.)..px(70.)]
    );
}
#[test]
fn combining_sequences_and_ligature_clusters_do_not_create_interior_carets() {
    let line = shaped("بَت", &[4, 0]);
    assert_eq!(line.closest_index_for_x(px(19.)), 0);
    assert_eq!(line.visual_caret(0, false), 4);
    let ligature = shaped("لا", &[0]);
    assert_eq!(ligature.x_for_index(0), px(10.));
    assert_eq!(ligature.x_for_index(2), px(5.));
    assert_eq!(ligature.x_for_index(4), px(0.));
}
struct Row {
    builds: Rc<Cell<usize>>,
    probes: Rc<RefCell<Vec<Bounds<Pixels>>>>,
}
impl Render for Row {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.builds.set(self.builds.get() + 1);
        let probes = self.probes.clone();
        div()
            .flex()
            .w(px(200.))
            .h(px(30.))
            .ps(px(12.))
            .child(
                canvas(
                    move |bounds, _, _| probes.borrow_mut().push(bounds),
                    |_, _, _, _| {},
                )
                .w(px(20.))
                .h(px(20.)),
            )
            .child(div().w(px(40.)).h(px(20.)))
    }
}
struct Parent {
    direction: LayoutDirection,
    row: Entity<Row>,
}
impl Render for Parent {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .w(px(200.))
            .h(px(30.))
            .direction(self.direction)
            .child(
                self.row
                    .clone()
                    .cached(StyleRefinement::default().w(px(200.)).h(px(30.))),
            )
    }
}
#[test]
fn cached_views_follow_direction_changes_and_resolve_logical_padding() {
    let mut cx = TestAppContext::single();
    let probes = Rc::new(RefCell::new(Vec::new()));
    let builds = Rc::new(Cell::new(0));
    let window = cx.add_window({
        let probes = probes.clone();
        let builds = builds.clone();
        move |_, cx| Parent {
            direction: LayoutDirection::Ltr,
            row: cx.new(|_| Row { probes, builds }),
        }
    });
    let viewport = cx
        .update_window(window.into(), |_, window, _| window.viewport_size().width)
        .unwrap();
    for (direction, x) in [
        (LayoutDirection::Ltr, px(12.)),
        (LayoutDirection::Rtl, viewport - px(32.)),
        (LayoutDirection::Ltr, px(12.)),
    ] {
        window
            .update(&mut cx, |parent, _, cx| {
                parent.direction = direction;
                cx.notify();
            })
            .unwrap();
        cx.update_window(window.into(), |_, window, cx| {
            window.draw(cx).clear(cx);
        })
        .unwrap();
        assert_eq!(probes.borrow().last().unwrap().origin.x, x);
    }
    assert!(
        builds.get() >= 3,
        "direction changes invalidate a retained view's context"
    );
}

#[test]
fn independently_shaped_rtl_row_carets_round_trip_at_both_edges() {
    let wrapped = crate::WrappedLineLayout {
        unwrapped_layout: std::sync::Arc::new(shaped("علي", &[4, 2, 0])),
        ..Default::default()
    };
    for (index, x) in [(0, 30.), (2, 20.), (4, 10.), (6, 0.)] {
        let caret = wrapped.position_for_index(index, px(20.)).unwrap();
        assert_eq!(
            caret,
            point(px(x), px(0.)),
            "RTL caret must stay on the physical row"
        );
        assert_eq!(
            wrapped
                .index_for_position(caret, px(20.))
                .unwrap_or_else(|ix| ix),
            index
        );
        assert_eq!(
            wrapped.closest_index_for_position(caret, px(20.)).unwrap(),
            index
        );
    }
    assert_eq!(wrapped.position_for_index(7, px(20.)), None);
}

#[test]
fn text_row_top_belongs_to_the_next_row() {
    assert!(!crate::fast::bidi::after_row(px(19.5), px(20.)));
    assert!(
        crate::fast::bidi::after_row(px(20.), px(20.)),
        "the shared edge must resolve to the following row, matching position_for_index"
    );
    assert!(crate::fast::bidi::after_row(px(20.5), px(20.)));
}

#[test]
fn source_text_preserves_hard_breaks_without_inserting_soft_wrap_bytes() {
    let lines = [
        ("مرحبا ", true),
        ("بالعالم", false),
        ("", false),
        ("abc", false),
    ]
    .map(|(text, fast_soft_wrap)| crate::WrappedLine {
        text: text.into(),
        fast_soft_wrap,
        layout: std::sync::Arc::new(crate::WrappedLineLayout::default()),
        decoration_runs: Vec::new(),
    });
    let text = crate::fast::bidi::source_text(&lines);
    assert_eq!(text, "مرحبا بالعالم\n\nabc");
    assert_eq!(
        text.len(),
        lines.iter().map(|line| line.text.len()).sum::<usize>() + 2
    );
    assert_eq!(crate::fast::bidi::source_text(&[]), "");
}

/// One sentence per probe, each under its own boundary.
struct Sentences {
    probes: Vec<(Option<LayoutDirection>, bool, &'static str)>,
    layouts: Rc<RefCell<Vec<crate::TextLayout>>>,
}
impl Render for Sentences {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let mut layouts = self.layouts.borrow_mut();
        layouts.clear();
        div().w(px(400.)).children(
            self.probes
                .iter()
                .map(|&(direction, logical_padding, text)| {
                    let text = crate::StyledText::new(text);
                    layouts.push(text.layout().clone());
                    let row = if logical_padding {
                        div().w_full().ps(px(4.))
                    } else {
                        div().w_full()
                    };
                    match direction {
                        Some(direction) => row.direction(direction),
                        None => row,
                    }
                    .child(text)
                }),
        )
    }
}

/// The caret at every character boundary, from the left edge of the text,
/// in tenths of a pixel.
fn carets(layout: &crate::TextLayout, text: &str) -> Vec<i32> {
    let left = layout.bounds().origin.x;
    text.char_indices()
        .map(|(index, _)| index)
        .chain([text.len()])
        .map(|index| {
            ((layout.position_for_index(index).unwrap().x - left) / px(0.1)).round() as i32
        })
        .collect()
}

#[test]
fn text_under_no_direction_boundary_takes_its_first_strong_base() {
    let arabic = "تجاهل الكل.";
    let latin = "Discard all.";
    let probes = vec![
        (None, false, arabic),
        (None, true, arabic),
        (Some(LayoutDirection::Ltr), false, arabic),
        (Some(LayoutDirection::Rtl), false, arabic),
        (None, false, latin),
        (Some(LayoutDirection::Ltr), false, latin),
    ];
    let mut cx = TestAppContext::single();
    let layouts = Rc::new(RefCell::new(Vec::new()));
    let window = cx.add_window({
        let layouts = layouts.clone();
        move |_, _| Sentences { probes, layouts }
    });
    cx.update_window(window.into(), |_, window, cx| {
        window.draw(cx).clear(cx);
        assert_eq!(window.layout_direction(), LayoutDirection::Ltr);
    })
    .unwrap();
    let layouts = layouts.borrow();
    let [unset, padded, ltr, rtl] = [0, 1, 2, 3].map(|i| carets(&layouts[i], arabic));
    assert_ne!(
        ltr, rtl,
        "the two bases must lay this Arabic out differently"
    );
    assert_eq!(
        unset, rtl,
        "Arabic under no boundary takes a right-to-left base, as under .rtl(): its full stop goes to the left"
    );
    assert_eq!(
        padded, rtl,
        "logical padding must not turn an unset direction into a left-to-right base"
    );
    assert_eq!(
        carets(&layouts[4], latin),
        carets(&layouts[5], latin),
        "Latin under no boundary keeps a left-to-right base"
    );
}
