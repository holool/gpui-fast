# Holool RTL support

This fork adds inherited LTR/RTL layout and bidirectional editing geometry to
Longbridge GPUI Fast. It remains based on Zed GPUI, not GPUI CE or Parley.

```rust
use gpui::{Directional as _, LayoutDirection, ParentElement as _, Styled as _, div, px};
let view = div().rtl().ps(px(12.)).child(div().ltr().child("source code"));
```

`Directional` supplies `direction(LayoutDirection::{Ltr,Rtl})`, `rtl()`, `ltr()`,
`ps`, `pe`, `ms`, `me`, `start`, and `end`. An unset direction inherits. Physical
`left`, `right`, `pl`, and `pr` keep their physical meanings. Source children,
focus traversal, accessibility order, and UTF-8 text remain in logical order;
Taffy 0.14 handles visual layout. A direction boundary defaults text alignment
to its inline start; an explicit physical text alignment wins.

Direction participates in retained view context equality, layout fingerprints,
and text measurement compatibility. The ordinary LTR borrowed-style path avoids
an additional style copy. Bidi rows are reshaped at grapheme-safe wrapping
boundaries; exponential search bounds the amount of prefix shaping. Soft wraps
are distinguished from actual newlines when mapping TextLayout byte offsets.

`WindowTextSystem::{shape_line_in_direction,shape_text_in_direction}` explicitly
set paragraph base direction. Their cache keys include an invisible leading
Unicode direction mark; it is removed from published text, runs, and byte
indices. Ordinary shaping keeps each paragraph's first-strong direction.
`LineLayout::{selection_ranges,visual_caret}` expose disjoint selection spans
and visual grapheme navigation. `LineLayout.fast_bidi` is optional shared
metadata: native constructors initialize it or leave it empty for the shared
shaping layer. CoreText uses native caret positions; cosmic-text uses cluster
advances; DirectWrite uses native hit-testing and run direction. Ligature caret
subdivision on cosmic-text falls back to equal grapheme shares when its shaper
provides only a cluster advance.

The X11 port preserves GPUI contact identities, routes updates to the original
window, cancels interrupted contacts, and suppresses emulated pointer events
from direct-touch devices on XI2 >= 2.2. The hardware-GPU preference from
Holool/gpui-arabic is already present in Fast and is not duplicated.

Run the focused checks:

```sh
cargo +1.95 test -p gpui --lib fast:: --features gpui_platform/runtime_shaders
cargo +1.95 test -p gpui_macos --lib --features font-kit,runtime_shaders native_arabic
cargo +1.95 test -p gpui_linux --features x11 fast::x11_touch
script/check-upstream
```

API compatibility: adding `TextStyle.fast_direction`, `Style.fast_logical`, and
`LineLayout.fast_bidi` requires downstream exhaustive struct literals to include
the new fields or use `..Default::default()`. Platform test-support exposes
`gpui_macos::text_system_for_tests()` for native shaping without an AppKit app.
Full CSS `direction:auto` layout inference and caret affinity at the two visual
positions of a mixed-direction boundary are not introduced by this patch.
Native Windows rendering and physical touchscreen gestures still require device
QA; unit tests and cross-compilation do not replace that check.
