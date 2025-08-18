use std::fs;
use std::path::Path;
use std::env;

fn main() {
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR environment variable not set");
    let dest_path = Path::new(&out_dir).join("config.rs");
    let default_config_path = Path::new("src/config.def.rs");
    
    // Tell Cargo to rerun if config.def.rs changes
    println!("cargo::rerun-if-changed=src/config.def.rs");
    
    // Only copy default config if config.rs doesn't exist in OUT_DIR
    if !dest_path.exists() {
        fs::copy(default_config_path, &dest_path)
            .expect("Failed to copy default config to OUT_DIR");
        println!("cargo::warning=Created config.rs from config.def.rs in OUT_DIR");
    }
}