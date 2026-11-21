//! Asset names and logos from Binance's website asset list.
//!
//! `GET https://www.binance.com/bapi/asset/v2/public/asset/asset/get-all-asset`
//! is the list binance.com itself uses (undocumented): ~1100 assets with
//! their full names ("Bitcoin", "TetherUS") and a logo URL on
//! `bin.bnbstatic.com` (small PNGs). Being undocumented it is best-effort:
//! the parser keeps whatever entries it can read and never panics on a
//! changed schema.

use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::time::Duration;

/// The asset list endpoint.
pub const ASSETS_URL: &str =
    "https://www.binance.com/bapi/asset/v2/public/asset/asset/get-all-asset";
/// The list is ~1.5 MB.
const ASSETS_TIMEOUT: Duration = Duration::from_secs(30);
const LOGO_TIMEOUT: Duration = Duration::from_secs(15);
/// Logos are ~1 KB; anything this big is not a logo.
pub const MAX_LOGO_BYTES: usize = 512 * 1024;

/// One asset of the list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetInfo {
    /// Upper-case asset code, as in symbol names ("BTC").
    pub code: String,
    /// Full name ("Bitcoin"); the code if the list has none.
    pub name: String,
    /// HTTPS URL of the logo image, if any.
    pub logo_url: Option<String>,
}

/// Parses the asset list. Entries without a readable code are skipped, the
/// first entry of a duplicated code wins. Fails only if there is no `data`
/// array at all (an error response or a different schema).
pub fn parse_assets(body: &str) -> Result<Vec<AssetInfo>> {
    let v: Value = serde_json::from_str(body).context("asset list is not JSON")?;
    let Some(items) = v.get("data").and_then(Value::as_array) else {
        let msg = v
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("no data array");
        bail!("unexpected asset list: {msg}");
    };
    let mut out: Vec<AssetInfo> = Vec::with_capacity(items.len());
    let mut seen = std::collections::HashSet::with_capacity(items.len());
    for item in items {
        let Some(a) = parse_asset(item) else { continue };
        if seen.insert(a.code.clone()) {
            out.push(a);
        }
    }
    Ok(out)
}

fn parse_asset(item: &Value) -> Option<AssetInfo> {
    let text = |key: &str| {
        item.get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
    };
    let code = text("assetCode")?.to_uppercase();
    let name = text("assetName")
        .or_else(|| text("assetDisplayName"))
        .unwrap_or(&code)
        .to_owned();
    let logo_url = [text("logoUrl"), text("fullLogoUrl")]
        .into_iter()
        .flatten()
        .find(|u| u.starts_with("https://"))
        .map(str::to_owned);
    Some(AssetInfo {
        code,
        name,
        logo_url,
    })
}

/// Downloads and parses the asset list.
pub async fn fetch_assets(http: &reqwest::Client) -> Result<Vec<AssetInfo>> {
    let resp = http
        .get(ASSETS_URL)
        .timeout(ASSETS_TIMEOUT)
        .send()
        .await
        .context("asset list request failed")?;
    let status = resp.status();
    let body = resp.text().await.context("reading asset list")?;
    if !status.is_success() {
        bail!("asset list: HTTP {status}");
    }
    parse_assets(&body)
}

/// Downloads one logo image (HTTPS only, size-capped, must look like an
/// image).
pub async fn fetch_logo(http: &reqwest::Client, url: &str) -> Result<Vec<u8>> {
    if !url.starts_with("https://") {
        bail!("logo URL is not HTTPS");
    }
    let resp = http
        .get(url)
        .timeout(LOGO_TIMEOUT)
        .send()
        .await
        .context("logo request failed")?;
    let status = resp.status();
    if !status.is_success() {
        bail!("logo: HTTP {status}");
    }
    if resp
        .content_length()
        .is_some_and(|n| n as usize > MAX_LOGO_BYTES)
    {
        bail!("logo too large");
    }
    let bytes = resp.bytes().await.context("reading logo")?;
    if bytes.len() > MAX_LOGO_BYTES {
        bail!("logo too large");
    }
    if !is_image(&bytes) {
        bail!("logo is not a PNG, JPEG or WebP image");
    }
    Ok(bytes.to_vec())
}

/// PNG, JPEG or WebP by magic bytes.
pub fn is_image(b: &[u8]) -> bool {
    b.starts_with(b"\x89PNG\r\n\x1a\n")
        || b.starts_with(&[0xFF, 0xD8, 0xFF])
        || (b.len() >= 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP")
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("fixtures/get_all_asset.json");

    #[test]
    fn parses_the_asset_list_tolerantly() {
        let list = parse_assets(FIXTURE).unwrap();
        let codes: Vec<&str> = list.iter().map(|a| a.code.as_str()).collect();
        assert_eq!(codes, ["BTC", "USDT", "ETH", "NOLOGO", "BLANK", "币安人生"]);
        let btc = &list[0];
        assert_eq!(btc.name, "Bitcoin");
        assert_eq!(
            btc.logo_url.as_deref(),
            Some("https://bin.bnbstatic.com/image/admin_mgs_image_upload/20220218/btc.png")
        );
        // `logoUrl` preferred over `fullLogoUrl`.
        assert!(list[1].logo_url.as_deref().unwrap().ends_with("usdt.png"));
        // Empty `logoUrl` falls back to `fullLogoUrl`; codes upper-cased.
        assert_eq!(
            list[2].logo_url.as_deref(),
            Some("https://bin.bnbstatic.com/image/eth.png")
        );
        assert_eq!(list[3].logo_url, None);
        // Blank name → display name; plain-HTTP logos are dropped.
        assert_eq!(list[4].name, "Blank Display");
        assert_eq!(list[4].logo_url, None);
    }

    #[test]
    fn schema_changes_are_errors_not_panics() {
        assert!(parse_assets("").is_err());
        assert!(parse_assets("[]").is_err());
        assert!(parse_assets(r#"{"code":"100","message":"busy","data":null}"#).is_err());
        assert!(parse_assets(r#"{"data":{}}"#).is_err());
        assert_eq!(parse_assets(r#"{"data":[]}"#).unwrap(), vec![]);
        assert_eq!(parse_assets(r#"{"data":[1,null,[],{}]}"#).unwrap(), vec![]);
    }

    #[test]
    fn image_magic() {
        assert!(is_image(b"\x89PNG\r\n\x1a\n...."));
        assert!(is_image(&[0xFF, 0xD8, 0xFF, 0xE0]));
        assert!(is_image(b"RIFF\0\0\0\0WEBPVP8 "));
        assert!(!is_image(b"<html>"));
        assert!(!is_image(b""));
    }
}
