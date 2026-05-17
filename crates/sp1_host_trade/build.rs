fn main() {
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_EXPENSIVE_PROOFS");
    #[cfg(feature = "expensive_proofs")]
    {
        let args = sp1_build::BuildArgs {
            binaries: vec!["radroots_sp1_trade_order_acceptance_guest".to_string()],
            features: vec!["sp1_guest".to_string()],
            locked: true,
            ..sp1_build::BuildArgs::default()
        };
        sp1_build::build_program_with_args("../sp1_guest_trade", args);
    }
}
