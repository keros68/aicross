fn main() {
    println!("cargo:rerun-if-changed=ui");
    // skill 内置包随 skill-bundle 变化重编（fix-50-r1）。
    println!("cargo:rerun-if-changed=skill-bundle");

    tauri_build::build()
}
