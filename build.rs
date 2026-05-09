fn main() {
    let data_dir = std::env::var("STEMBOT_DATA_DIR").unwrap_or_default();
    println!("cargo:rustc-env=STEMBOT_DATA_DIR={}", data_dir);
    println!("cargo:rerun-if-env-changed=STEMBOT_DATA_DIR");
}
