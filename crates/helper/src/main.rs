#![windows_subsystem = "windows"]
fn main() {
    if let Err(error) = dustaway_windows::ipc::serve_helper() {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}
