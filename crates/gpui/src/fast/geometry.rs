//! Compiler-compatible implementation of GPUI's golden-ratio helper.
pub(crate) const fn phi() -> crate::DefiniteLength {
    crate::relative(std::f32::consts::GOLDEN_RATIO)
}
