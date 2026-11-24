fn main() {
    // skill 内置包随 skill-bundle 变化重编。
    println!("cargo:rerun-if-changed=skill-bundle");

    #[cfg(feature = "tauri-ui")]
    tauri_build::build();
}
