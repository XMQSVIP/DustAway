//! Explicit read-only evidence; never starts an application or invokes deletion.
fn main() -> anyhow::Result<()> {
    let platform = dustaway_windows::WindowsPlatform::new(dustaway_core::Scope::User)?;
    let result = dustaway_windows::application_evidence::scan(
        &platform,
        &dustaway_windows::applications::support(),
    )?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
