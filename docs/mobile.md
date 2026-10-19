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
rustup target add aarch64-apple-ios aarch64-apple-ios-sim aarch64-linux-android wasm32-unknown-unknown
cargo check -p gpui_apple --target aarch64-apple-ios-sim --features font-kit,runtime_shaders
cargo check -p gpui_apple --target aarch64-apple-ios --features font-kit,runtime_shaders
cargo check -p gpui_wgpu --target aarch64-linux-android
cargo check -p gpui_wgpu --target wasm32-unknown-unknown --no-default-features
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

## Android graphics backends

An Android native window can have only one graphics API connection. The private
`fast::mobile::instance` hook chooses one backend before creating a surface;
creating both Vulkan and EGL surfaces for one `ANativeWindow` produced
`native_window_api_connect: already connected` / `EGL_BAD_ALLOC` in the emulator.
Other native platforms retain the original Vulkan/GL instance behavior.

Android prefers Vulkan when an adapter is available. Known Goldfish/ranchu
emulator Vulkan identities use GLES because the observed driver creates a device
but immediately loses it while the renderer creates its atlas. Missing Vulkan
also selects GLES. Physical Vulkan adapter selection is unchanged; physical
Android GPUs still need qualification.

Android GLES reuses the renderer's existing WebGL2 path that stores drawing
instances in textures, requests GLES-compatible limits without compute/storage
buffers, and prefers an RGBA atlas. This preserves the shared scene, scroll
layers and text system. The browser path retains its existing behavior. The
mobile workflow also checks `gpui_wgpu` for `wasm32-unknown-unknown`, since the
compatibility helpers are shared with WebGL.

Three focused tests cover GLES limits, native Vulkan/desktop GL requirements,
and the emulator identity exclusion. On an arm64 API36.1 emulator, the Android
backend rendered Arabic and mixed-direction text, scrolled through raw touch,
activated the software keyboard, and resumed drawing after Home/foreground.
Real InputConnection instrumentation exercised Arabic composition, UTF-16
selection, and both-side/codepoint deletion. These results do not qualify
physical Vulkan drivers, real Arabic keyboards, TalkBack, color emoji, or long
sustained sessions.

### Vulkan debug labels

The Android emulator's Goldfish Vulkan driver crashed in
`vk_common_SetDebugUtilsObjectNameEXT` when WGPU assigned debug object names.
The private `fast::mobile::instance_flags` hook removes `InstanceFlags::DEBUG`
on Android and retains the other default flags, including validation in debug
builds. Both initial instance creation and GPU recovery use the same hook.
Other native platforms keep WGPU's default flags.
