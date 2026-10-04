//! Read-only support inventory. Does not scan personal files or invoke deletion.
fn main() {
    println!(
        "{}",
        serde_json::to_string_pretty(&dustaway_windows::application_evidence::inventory(
            &dustaway_windows::applications::support()
        ))
        .unwrap()
    );
}
