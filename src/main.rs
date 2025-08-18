mod noice;

fn main() {
    let dir = std::env::args().nth(1)
        .unwrap_or_else(|| ".".to_string());
    
    if let Err(e) = noice::run(&dir) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}