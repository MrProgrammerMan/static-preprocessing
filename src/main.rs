use std::path::Path;

fn main() {
    let _ = static_preprocessing::process_directory(Path::new("static-test-files"), Path::new("dest"));
}