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

## October 2026 upstream sync

The sync through longbridge/gpui-fast `eeccb27f7b7b437e0d99277af2b696e6c71fb6ef`
retains Holool's inherited direction, logical edges, bidi caret/selection geometry,
independently shaped wrapped rows, accessibility state extensions, and X11 touch
contacts (including the window-open borrow fix). It includes upstream's Zed
`93d7456688` import and the `gpui-pre` 0.3.9 compatibility snapshot.

CoreText now lives in `gpui_apple`. Native bidi geometry and Arabic regression
coverage moved with it; `gpui_macos::text_system_for_tests` remains available to
existing Kit tests as a forwarding helper.

Upstream's unconstrained text-measurement cache guard must also check
`WrappedLine.fast_soft_wrap`: independently shaped bidi rows have no internal
wrap boundaries. Without that check, a narrow Arabic measurement incorrectly
answers a later MaxContent/MinContent probe. The upstream regression now runs
with Latin, Arabic, and mixed text, and reproduced that failure before the fix.

Holool consumes these extensions through Git revisions. The upstream crates.io
publication rehearsal resolves the official published core, which lacks
Holool's bidi fields and APIs, so that rehearsal and publication run only in
`longbridge/gpui-fast`. Fork CI still checks prepared release workspaces,
all-feature/backend compilation, web compilation, package contents, release
order, and the normal native test matrix. It does not claim that Holool's code
can be published under upstream's crate names and versions.


## October 20 upstream sync and empty directional text

The sync through longbridge/gpui-fast `b836dd970b26830f2124b7cf91b0e19018fd12da`
includes the Zed `b4ecc45422` import, retained-row state fixes, accurate retained
frame debug bounds and scroll translations, rounded-overflow clipping, and wgpu
30. Holool's direction inheritance, native bidi geometry, and native mobile
hooks remain in place.

CE commit `337a8d3969d5478` fixes empty source text being made nonempty by bidi
preprocessing without font runs. Fast uses CoreText, Cosmic and DirectWrite;
it does not use CE's Parley code. Its existing `shape_line_in_direction` guard
already checks the original empty source before inserting direction marks. This
sync documents that invariant and adds native CoreText and font-isolated Cosmic
regressions for empty input with no runs and zero-length runs, both directions,
fresh/cached layouts, and wrapped/unwrapped shaping. Empty source lengths,
zero advance, empty glyphs and caret byte offsets must remain unchanged.

The source dependency retains upstream's wgpu Git revision
`3b3506ea31154edc04de64debda0e780cba2639c`, which contains the Vulkan surface-drop
fix absent from published wgpu 30.0.1. Android's single-API surface selection,
GLES-compatible instance textures/limits, emulator Vulkan exclusion and debug
label workaround are preserved. Upstream's new in-tree iOS crate does not replace
Holool's separately maintained `gpui_ios` native host adapter.


`TextLayout::text()` reconstructs logical source text while omitting synthetic
separators between independently shaped RTL soft rows. This corrects an
existing source getter without adding an API. Kit's new immutable selection
and editor-host snapshots need source ranges, rather than assuming every
shaped line ends in a newline; their native regression coverage checks both
hard and soft boundaries against CoreText's original caret positions.
