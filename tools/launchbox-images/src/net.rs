//! HTTP downloads (ureq with rustls).

use std::fs::{self, File};
use std::io::{self, BufWriter, IsTerminal, Read, Write};
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::Context;

const USER_AGENT: &str = concat!(
    "armory-launchbox-images/",
    env!("ARMORY_VERSION"),
    " (+https://github.com/tbreckle/armory)"
);
/// Largest image accepted by `fetch`; clear logos are a few MB at most.
const IMAGE_LIMIT: u64 = 64 << 20;

fn get(url: &str) -> anyhow::Result<ureq::http::Response<ureq::Body>> {
    ureq::get(url)
        .header("User-Agent", USER_AGENT)
        .call()
        .with_context(|| format!("cannot download {url}"))
}

/// Downloads `url` into `target` with a progress display, via a temporary file next to it so
/// that `target` is never half written.
pub fn download(url: &str, target: &Path) -> anyhow::Result<()> {
    let mut response = get(url)?;
    let total = response.body().content_length();
    let part = target.with_extension("part");
    let result = (|| -> io::Result<()> {
        let mut out = BufWriter::new(File::create(&part)?);
        let mut reader = response.body_mut().as_reader();
        let progress = io::stderr().is_terminal();
        let mut buf = vec![0; 1 << 16];
        let (mut done, mut shown) = (0u64, Instant::now());
        loop {
            let n = reader.read(&mut buf)?;
            if n == 0 {
                break;
            }
            out.write_all(&buf[..n])?;
            done += n as u64;
            if progress && shown.elapsed() > Duration::from_millis(500) {
                shown = Instant::now();
                let percent = total.map(|t| format!(" {}%", done * 100 / t.max(1)));
                eprint!("\r  {} MB{}", done >> 20, percent.unwrap_or_default());
            }
        }
        if progress {
            eprintln!("\r  {} MB      ", done >> 20);
        }
        out.into_inner().map_err(|e| e.into_error())?.sync_all()?;
        fs::rename(&part, target)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&part);
    }
    result.with_context(|| format!("cannot download {url} to {}", target.display()))
}

/// Downloads `url` into memory.
pub fn fetch(url: &str) -> anyhow::Result<Vec<u8>> {
    get(url)?
        .body_mut()
        .with_config()
        .limit(IMAGE_LIMIT)
        .read_to_vec()
        .with_context(|| format!("cannot download {url}"))
}
