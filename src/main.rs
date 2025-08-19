mod noice;
use std::env;
use std::process;

fn print_usage() {
    eprintln!("Usage: noice-rs [-ct] [-f SAVE_FILE] [DIR]");
    eprintln!("Options:");
    eprintln!("  -c    Enable color mode");
    eprintln!("  -t    Enable tilde home display");
    eprintln!("  -f    Use alternate save file");
    process::exit(1);
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut dir = None;
    let mut color_mode = true; // Default to color on
    let mut tilde_home = false;
    let mut save_file = None;
    let mut i = 1;
    
    while i < args.len() {
        let arg = &args[i];
        if arg.starts_with('-') && arg != "-" {
            for ch in arg.chars().skip(1) {
                match ch {
                    'c' => color_mode = true,
                    't' => tilde_home = true,
                    'f' => {
                        i += 1;
                        if i >= args.len() {
                            eprintln!("Option -f requires an argument");
                            print_usage();
                        }
                        save_file = Some(args[i].clone());
                        break;
                    }
                    _ => {
                        eprintln!("Unknown option: -{ch}");
                        print_usage();
                    }
                }
            }
        } else if dir.is_none() {
            dir = Some(arg.clone());
        } else {
            eprintln!("Too many arguments");
            print_usage();
        }
        i += 1;
    }
    
    let dir = dir.unwrap_or_else(|| ".".to_string());
    
    if let Err(e) = noice::run(&dir, color_mode, tilde_home, save_file) {
        eprintln!("Error: {e}");
        process::exit(1);
    }
}