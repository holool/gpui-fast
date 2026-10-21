# ADR: text with no direction set takes its base direction from its first strong character

**Date:** 2026-10-21
**Status:** Proposed
**Code at:** `main` `4c98f72` (the text paths are unchanged from `c4e8f3c`, the
revision GPUI Kit `4d4f999` pins).

## Context

An application that never sets a direction gets every Arabic line laid out
on an LTR paragraph base. Arabic letters still read right to left inside
their run. A neutral or weak character at either end of the line is placed
on the wrong side:

- ".تتلقى الصناديق القائمة الجديدة فور النشر" (the period should be at the
  left);
- "…تجاهل الكل" (the ellipsis should be at the left);
- "3 ضيوف" written as `"{guests} ضيوف"`: the number goes to the left, where
  it should be at the right.

A leading LTR isolate (`"\u{2066}{n}\u{2069} معلقة"`) and a Latin word at an
edge do the same.

Seen in Sadeed's restaurant app, which uses GPUI Kit on this runtime. That
app mirrors its layout by hand, because a `.rtl()` boundary also mirrors
Taffy's flex and block layout. So its Arabic text sits under the default
LTR direction everywhere.

**Cause.**

- `crates/gpui/src/fast/text.rs`, in text measurement: any text that
  contains an RTL character is shaped by `shape_text_in_direction(..,
  text_style.fast_direction)`. `TextStyle.fast_direction`
  (`crates/gpui/src/style.rs`) is a plain `LayoutDirection` that defaults to
  `Ltr`, so a direction nobody set reads the same as an explicit `.ltr()`.
- `crates/gpui/src/fast/bidi.rs`, `shape_line_in_direction`: in an LTR
  context it puts U+200E LEFT-TO-RIGHT MARK in front of every row.
  `shape_text_in_direction` shapes each wrapped row on its own.
- The plain path, `WindowTextSystem::shape_text` through `shape_text_auto`,
  already picks each paragraph's direction from its first strong character.
  [`holool-rtl.md`](../holool-rtl.md) says so ("Ordinary shaping keeps each
  paragraph's first-strong direction"). Element text does not get that
  behaviour.

**Why the application cannot fix it.** Checked in the Arabic snapshot of the
Sadeed restaurant app's Menu Review dialog, 2026-10-21:

| Change in the application | Result |
|---|---|
| Text prefixed with U+200F RIGHT-TO-LEFT MARK | No change. The runtime's LRM comes first and sets the base. |
| Text wrapped in U+2067…U+2069 (RLI…PDI) | Fixed for one-row text only. A wrapped row 2 has no RLI. |
| `.rtl()` on the element that holds the text | Fixed, but only element by element. On a container it also mirrors the rows inside. |

Marks in the strings also reach values the application stores or exports:
names typed into inputs, and CSV headers. The application has no single
place to give all of its text an RTL base. Neither does GPUI Kit, whose
components hold their own text elements (Button label, Dialog title and
description, Tag, Tab, Select).

## Decision

When neither the text nor any ancestor has set a direction, element text
takes each paragraph's base direction from its first strong character, as
`shape_text` already does. An explicit `.ltr()` or `.rtl()` keeps forcing
its direction. Layout direction (Taffy) does not change.

| Context | Text | Base today | Proposed |
|---|---|---|---|
| no direction set | Arabic first | LTR | RTL |
| no direction set | Latin first | LTR | LTR |
| `.ltr()` boundary | Arabic first | LTR | LTR |
| `.rtl()` boundary | any | RTL | RTL |

## Constraints (CLAUDE.md, docs/upstream-sync.md)

- No new public API. `TextStyle.fast_direction` keeps its type and its
  meaning for layout.
- The logic lives in `crates/gpui/src/fast/`. Upstream files get hooks at
  most, and `script/check-upstream` passes.
- The open design point is how text measurement tells "unset" from
  "inherited LTR" without a public field. One candidate: `fast/text_style.rs`
  records, in `TextStyleStack`, whether any pushed refinement set
  `fast_direction`, and `fast/text.rs` reads that crate-private flag. This
  only works if a refinement pushed for children carries the element's own
  `direction(..)` and not the value `fast/direction.rs::inherit_style` fills
  in for layout. That must be confirmed before implementing. If it fails, the
  flag has to live in the fast part of the style, not in the refinement.
- Measurement caching: text-measure compatibility already compares
  `fast_direction`. The new flag must take part in that comparison, so that
  cached layouts do not mix the two behaviours.

## Consequences

- Applications and Kit components that set no direction show Arabic, Hebrew
  and mixed text in the correct reading order, including wrapped rows. Their
  strings and stored values need no bidi marks.
- An application that relied on Arabic text being forced to LTR without
  saying so must now add `.ltr()`. The case looks rare; it is a behaviour
  change all the same, so it needs a note in `holool-rtl.md` and in the
  release notes.
- Caret and selection geometry already handle RTL rows
  (`LineLayout::{selection_ranges, visual_caret}`), so editable text gets
  the same base without new geometry code. Inputs normally set their own
  direction anyway.
- Downstream: a Kit revision bump, then an application lock bump.

## Verification

- `fast/tests/bidi.rs`, all asserted on glyph x positions:
  - with no direction set, the trailing period of an Arabic line is at the
    left;
  - the same holds on the last row of a wrapped Arabic paragraph;
  - Latin text is unchanged;
  - under `.ltr()` the period stays at the right.

  Each test must fail when the change is reverted.
- The native shaping regressions in
  [`holool-rtl.md`](../holool-rtl.md) "Run the focused checks"
  (`cargo +1.95 test -p gpui --lib fast:: …`, the CoreText and cosmic-text
  Arabic tests) and `script/check-upstream` pass.
