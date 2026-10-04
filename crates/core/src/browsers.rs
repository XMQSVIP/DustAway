//! Built-in Chromium products. Paths are relative to the current user's LocalAppData.
pub struct ChromiumProduct {
    pub id: &'static str,
    pub user_data: &'static str,
    pub process: &'static str,
}

pub const CHROMIUM_PRODUCTS: &[ChromiumProduct] = &[
    ChromiumProduct {
        id: "chrome",
        user_data: "Google/Chrome/User Data",
        process: "chrome.exe",
    },
    ChromiumProduct {
        id: "edge",
        user_data: "Microsoft/Edge/User Data",
        process: "msedge.exe",
    },
    ChromiumProduct {
        id: "brave",
        user_data: "BraveSoftware/Brave-Browser/User Data",
        process: "brave.exe",
    },
    ChromiumProduct {
        id: "vivaldi",
        user_data: "Vivaldi/User Data",
        process: "vivaldi.exe",
    },
];

pub const CHROMIUM_CACHES: &[(&str, &str)] = &[
    ("http", "Cache/Cache_Data"),
    ("code", "Code Cache"),
    ("gpu", "GPUCache"),
];

pub fn chromium(id: &str) -> Option<&'static ChromiumProduct> {
    CHROMIUM_PRODUCTS.iter().find(|product| product.id == id)
}

pub fn standard_profile(name: &str) -> bool {
    name == "Default"
        || name
            .strip_prefix("Profile ")
            .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
}
