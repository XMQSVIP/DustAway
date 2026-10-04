fn main() -> anyhow::Result<()> {
    let scope = if std::env::args().any(|a| a == "--system") {
        dustaway_core::Scope::System
    } else {
        dustaway_core::Scope::User
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&dustaway_windows::system_maintenance::inspect(scope))?
    );
    Ok(())
}
