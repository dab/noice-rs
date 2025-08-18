use std::fs;
use std::path::Path;

fn main() {
    let config_path = Path::new("config.rs");
    let default_config_path = Path::new("src/config.def.rs");
    
    if !config_path.exists() && default_config_path.exists() {
        fs::copy(default_config_path, config_path)
            .expect("Failed to copy default config");
        println!("cargo:warning=Created config.rs from config.def.rs");
    }
}