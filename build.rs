fn main() {
    let out = std::env::var("OUT_DIR").unwrap();
    // OUT_DIR is deep inside target/debug/build/iris-xxx/out
    // going up 3 ancestors gives us target/debug/
    let target_dir = std::path::Path::new(&out)
        .ancestors()
        .nth(3)
        .unwrap()
        .to_path_buf();

    let src = std::path::Path::new("src/python/iris_bridge.py");
    let dst = target_dir.join("iris_bridge.py");
    std::fs::copy(src, dst).expect("Failed to copy iris_bridge.py");

    println!("cargo:rerun-if-changed=src/python/iris_bridge.py");
}