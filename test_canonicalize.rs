use std::path::PathBuf;

fn main() {
    let path = PathBuf::from(r"\\?\C:\Windows");
    match path.canonicalize() {
        Ok(p) => println!("Success: {}", p.display()),
        Err(e) => println!("Error: {}", e),
    }
}