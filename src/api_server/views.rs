//! HTML view rendering.
//!
//! The wallet dashboard is split into focused source files:
//! - `templates/wallet.html` — markup skeleton with `{{STYLE}}` / `{{SCRIPT}}` slots
//! - `static/wallet.css`     — all styling
//! - `static/wallet.js`      — all browser behaviour (same endpoints, no logic change)
//!
//! They are composed at compile time via `include_str!`, so `/wallet`
//! stays a single self-contained response with no extra routes or runtime I/O.

const WALLET_TEMPLATE: &str = include_str!("templates/wallet.html");
const WALLET_CSS: &str = include_str!("static/wallet.css");
const WALLET_JS: &str = include_str!("static/wallet.js");

/// Render the full `/wallet` HTML page.
pub fn render_wallet_page() -> String {
    WALLET_TEMPLATE
        .replace("{{STYLE}}", WALLET_CSS)
        .replace("{{SCRIPT}}", WALLET_JS)
}
