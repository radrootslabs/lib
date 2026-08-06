#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

#[cfg_attr(coverage_nightly, coverage(off))]
fn main() {
    uniffi::uniffi_bindgen_main();
}
