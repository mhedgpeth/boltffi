fn main() {
    let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    
    cbindgen::Builder::new()
        .with_crate(&crate_dir)
        .with_config(cbindgen::Config::from_root_or_default(&crate_dir))
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file("bindings.h");
    
    println!("cargo:rerun-if-changed=src/");
}
