fn main() {
    // rust-embed 在编译期读取 ../frontend/build；纯后端 CI（fmt/clippy/test）不会先构建前端，
    // 目录缺失会直接编译失败。这里保证目录存在（此时内嵌为空集，运行时优雅降级为提示页）。
    // 正式打包走 tauri 的 beforeBuildCommand，目录一定有真实产物。
    let dist = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../frontend/build");
    if !dist.exists() {
        let _ = std::fs::create_dir_all(&dist);
    }
    println!("cargo:rerun-if-changed=../frontend/build");

    tauri_build::build()
}
