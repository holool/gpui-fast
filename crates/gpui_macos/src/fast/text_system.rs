//! Preserve Holool's macOS test helper after CoreText moved to gpui_apple.

pub fn text_system_for_tests() -> std::sync::Arc<dyn gpui::PlatformTextSystem> {
    gpui_apple::text_system_for_tests()
}
