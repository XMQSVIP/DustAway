//! Read-only real-layout probe. Never prepares or executes deletion.
use anyhow::Result;
use dustaway_core::*;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
fn main() -> Result<()> {
    let engine = AttachmentEngine::new(Arc::new(dustaway_windows::attachments::WindowsAttachments));
    let provider = match std::env::var("DUSTAWAY_ATTACHMENT_PROVIDER").as_deref() {
        Ok("qq_classic") => AttachmentProvider::QqClassic,
        Ok("qqnt") => AttachmentProvider::Qqnt,
        _ => AttachmentProvider::Weixin,
    };
    let discovery = engine.discover(provider);
    let locations = &discovery.locations;
    let location = if let Some(path) = std::env::args().nth(1) {
        Some(engine.register(provider, &path)?)
    } else {
        locations.first().cloned()
    };
    let mut output = serde_json::json!({"discovery":discovery, "deletion_attempted":false});
    if let Some(location) = location {
        let session = engine.start(&location.token, vec![])?;
        let started = Instant::now();
        while engine.is_busy() && started.elapsed() < Duration::from_secs(90) {
            std::thread::sleep(Duration::from_millis(100));
        }
        if engine.is_busy() {
            engine.cancel();
        }
        let page = engine.page(AttachmentQuery {
            session: session.clone(),
            kind: None,
            min_age_days: 0,
            search: String::new(),
            offset: 0,
            selected_ids: None,
        })?;
        let mut previews = vec![];
        for entry in page.entries.iter().filter(|e| e.preview_available).take(2) {
            let result = engine.thumbnail(&session, &entry.id);
            previews.push(serde_json::json!({"name":entry.name,"bytes":result.as_ref().map(|s| s.len()).ok(),"error":result.err().map(|e| e.to_string())}));
        }
        output["page"] = serde_json::to_value(page)?;
        output["previews"] = serde_json::to_value(previews)?;
    }
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}
