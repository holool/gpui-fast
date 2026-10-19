# Native mobile foundations

Holool's mobile backends live in [gpui_ios](https://github.com/holool/gpui_ios)
and [gpui_android](https://github.com/holool/gpui_android). They implement the
current `Platform` and `PlatformWindow` contracts against this runtime. Native
hosts own application startup and pass execution into the platform backend.
Do not replace this core with the mobile donor's older GPUI snapshot.

The iOS backend uses `gpui_apple::AppleTextSystem` and
`gpui_apple::metal_renderer::MetalRenderer::from_layer`. Android uses
`gpui_wgpu::CosmicTextSystem` and `WgpuRenderer`. Keeping those implementations
preserves Holool's bidi geometry, cached scroll layers, and renderer texture-ID
contracts. The donor's renderer and shapers are not compatible replacements.

Native contacts enter the existing `TouchEvent` and gesture recognizers. A host
must not also synthesize mouse clicks or scroll/fling events for the same
contact. Source text stays in logical UTF-8 order; native input adapters convert
UTF-16 ranges at their boundary. Physical touch coordinates are not mirrored
when a view uses RTL layout.

All consumers must resolve the same Fast revision. GPUI Kit's `gpui-pre-*`
compatibility packages re-export this core; they are not a second runtime.
Application lifetime follows `Application::run_embedded`: a native host must
retain its `ApplicationHandle` while callbacks can reach GPUI.

## Build checks

```sh
rustup target add aarch64-apple-ios aarch64-apple-ios-sim aarch64-linux-android
cargo check -p gpui_apple --target aarch64-apple-ios-sim --features font-kit,runtime_shaders
cargo check -p gpui_apple --target aarch64-apple-ios --features font-kit,runtime_shaders
cargo check -p gpui_wgpu --target aarch64-linux-android
script/check-upstream
```

The Mobile foundations workflow checks these shared libraries. Linking native
hosts, signing and packaging belong to the backend repositories. The common
SQLite workload and GPUI/Flutter applications live in
[gpui-mobile-lab](https://github.com/holool/gpui-mobile-lab).

Compilation is not device validation. Native Arabic composition, diacritics,
mixed-direction caret/selection, clipboard, safe areas, rotation, surface
recreation, background/foreground recovery, VoiceOver/TalkBack and sustained
rendering require target-device acceptance. See each backend's validation record
for checks actually performed; do not infer a device result from desktop tests.
