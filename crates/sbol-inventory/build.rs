use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let rules = manifest_dir.join("profile/0.2/rules.toml");
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR set by Cargo"));

    sbol_rulegen::generate_profile(&rules, &out_dir);

    println!("cargo:rerun-if-changed=profile/0.2/rules.toml");
    println!("cargo:rerun-if-changed=profile/0.2/fixtures");
    println!("cargo:rerun-if-changed=build.rs");
}
