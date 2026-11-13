//! Zendo's HTTP: one user agent for every request, a timeout on each, and
//! downloads streamed into a file.

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::{Context, bail};
use ureq::config::ConfigBuilder;
use ureq::typestate::AgentScope;

/// How Zendo names itself to the sites it asks.
pub(crate) const USER_AGENT: &str = concat!(
    "zendo/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/leandrop-dev/zendo)"
);

/// The longest a download waits for its next bytes before it gives up.
const STALL: Duration = Duration::from_secs(60);

fn builder() -> ConfigBuilder<AgentScope> {
    ureq::Agent::config_builder().user_agent(USER_AGENT)
}

/// An agent for calls answered whole within `timeout`.
pub(crate) fn agent(timeout: Duration) -> ureq::Agent {
    builder().timeout_global(Some(timeout)).build().into()
}

/// [`agent`], where an error status (a refused key, a missing game) is an
/// answer to read rather than a failure.
pub(crate) fn answering_agent(timeout: Duration) -> ureq::Agent {
    builder()
        .timeout_global(Some(timeout))
        .http_status_as_error(false)
        .build()
        .into()
}

/// An agent for downloads, which may take long: it waits a while to
/// connect and for the answer to start, then gives up when the bytes stop
/// coming for a minute, so a stalled download ends and Stop is heard.
pub(crate) fn download_agent() -> ureq::Agent {
    builder()
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_recv_response(Some(Duration::from_secs(30)))
        .timeout_per_read(Some(STALL))
        .build()
        .into()
}

/// Downloads `url` into `to`, saying how far it got every 4 MiB, against
/// `size` or the length the server gives. `stop` ends it between reads;
/// `what` names it in errors. Leaves what it wrote on failure.
///
/// # Errors
///
/// When the download cannot start, breaks off, stalls, is stopped, ends
/// short of its length, or cannot be written.
pub(crate) fn download(
    url: &str,
    to: &Path,
    what: &str,
    size: Option<u64>,
    stop: &AtomicBool,
    progress: &mut dyn FnMut(u64, Option<u64>),
) -> anyhow::Result<()> {
    let mut response = download_agent()
        .get(url)
        .call()
        .with_context(|| format!("could not download {what}"))?;
    let total = size.or_else(|| response.body().content_length());
    let mut body = response.body_mut().with_config().limit(u64::MAX).reader();
    let mut file = File::create(to).with_context(|| format!("could not write {}", to.display()))?;
    let mut buffer = vec![0_u8; 256 * 1024];
    let mut done = 0_u64;
    let mut told = 0_u64;
    progress(0, total);
    loop {
        if stop.load(Ordering::Relaxed) {
            bail!("stopped");
        }
        let read = body
            .read(&mut buffer)
            .with_context(|| format!("the download of {what} broke off"))?;
        if read == 0 {
            break;
        }
        file.write_all(&buffer[..read])
            .with_context(|| format!("could not write {}", to.display()))?;
        done += u64::try_from(read).unwrap_or(u64::MAX);
        // A word every 4 MiB is plenty for a bar.
        if done - told >= 4 * 1024 * 1024 {
            told = done;
            progress(done, total);
        }
    }
    file.flush()?;
    if let Some(total) = total
        && done != total
    {
        bail!("the download of {what} ended at {done} of {total} bytes");
    }
    progress(done, Some(done));
    Ok(())
}

/// A URL's query part decoded: `%XX` as that byte, `+` as a space; a `%`
/// without two hex digits after it stays.
pub(crate) fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while let Some(&byte) = bytes.get(at) {
        let hex = bytes
            .get(at + 1..at + 3)
            .and_then(|hex| std::str::from_utf8(hex).ok())
            .and_then(|hex| u8::from_str_radix(hex, 16).ok());
        match (byte, hex) {
            (b'%', Some(decoded)) => {
                out.push(decoded);
                at += 3;
            }
            (b'+', _) => {
                out.push(b' ');
                at += 1;
            }
            (other, _) => {
                out.push(other);
                at += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The image kinds Zendo keeps, by extension and first bytes.
const IMAGE_KINDS: [(&str, &[u8]); 4] = [
    ("jpg", b"\xFF\xD8\xFF"),
    ("png", b"\x89PNG"),
    ("gif", b"GIF8"),
    ("webp", b"RIFF"),
];

/// The extension for an image's bytes; `None` for anything else (an error
/// page, say).
fn image_kind(bytes: &[u8]) -> Option<&'static str> {
    IMAGE_KINDS
        .iter()
        .find(|(_, magic)| bytes.starts_with(magic))
        .map(|(extension, _)| *extension)
}

/// A small image's file in `dir`, downloaded when not there yet. Named
/// after its address, with the extension its bytes call for, as the window
/// reads an image's kind from its name (Epic's addresses have none).
/// `None` when it cannot be downloaded or is not an image.
pub(crate) fn cached_image(dir: &Path, url: &str) -> Option<std::path::PathBuf> {
    let name = crate::paths::safe_name(url.split_once("://").map_or(url, |(_, rest)| rest));
    let kept = IMAGE_KINDS
        .iter()
        .map(|(extension, _)| dir.join(format!("{name}.{extension}")))
        .find(|file| file.is_file());
    if kept.is_some() {
        return kept;
    }
    let bytes = agent(Duration::from_secs(20))
        .get(url)
        .call()
        .ok()?
        .body_mut()
        .with_config()
        .limit(2 * 1024 * 1024)
        .read_to_vec()
        .ok()?;
    let file = dir.join(format!("{name}.{}", image_kind(&bytes)?));
    crate::paths::write_atomic(&file, &bytes).ok()?;
    Some(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_an_image_by_its_bytes() {
        assert_eq!(image_kind(b"\xFF\xD8\xFF\xE0rest"), Some("jpg"));
        assert_eq!(image_kind(b"\x89PNG\r\n"), Some("png"));
        assert_eq!(image_kind(b"<html>"), None);
    }

    #[test]
    fn decodes_query_values() {
        assert_eq!(percent_decode("a%20b+c%2"), "a b c%2");
        assert_eq!(percent_decode("%E2%9C%93%zz"), "✓%zz");
        assert_eq!(percent_decode(""), "");
    }
}
