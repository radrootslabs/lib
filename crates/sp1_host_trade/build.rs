fn main() {
    let build_guest_elf_env = "RADROOTS_SP1_HOST_TRADE_BUILD_GUEST_ELF";
    let run_real_proof_tests_env = "RADROOTS_SP1_HOST_TRADE_RUN_REAL_PROOF_TESTS";
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_SP1_VERIFY");
    println!("cargo:rerun-if-env-changed={build_guest_elf_env}");
    println!("cargo:rerun-if-env-changed={run_real_proof_tests_env}");
    println!("cargo:rustc-check-cfg=cfg(radroots_sp1_guest_elf)");
    println!("cargo:rustc-check-cfg=cfg(radroots_sp1_real_proof_tests)");
    if std::env::var(run_real_proof_tests_env).as_deref() == Ok("1") {
        println!("cargo:rustc-cfg=radroots_sp1_real_proof_tests");
    }
    if std::env::var(build_guest_elf_env).as_deref() != Ok("1") {
        return;
    }
    #[cfg(feature = "sp1_verify")]
    {
        let args = sp1_build::BuildArgs {
            binaries: vec!["radroots_sp1_trade_order_acceptance_guest".to_string()],
            features: vec!["sp1_guest".to_string()],
            locked: true,
            ..sp1_build::BuildArgs::default()
        };
        sp1_build::build_program_with_args("../sp1_guest_trade", args);
        println!("cargo:rustc-cfg=radroots_sp1_guest_elf");
    }
}
