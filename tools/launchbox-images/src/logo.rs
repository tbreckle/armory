//! Downloading a game's clear logo into the images folder as PNG.

use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};

use crate::index::Index;
use crate::search::{pick_logo, slug};

pub const IMAGE_URL: &str = "https://images.launchbox-app.com/";
const PNG_MAGIC: &[u8] = b"\x89PNG\r\n\x1a\n";

pub struct Request<'a> {
    pub id: u64,
    pub out_dir: &'a Path,
    /// Preferred region, empty for the default order.
    pub region: &'a str,
    /// File name instead of the game's name, empty for the game's name.
    pub name: &'a str,
    /// Replace an existing file.
    pub force: bool,
}

/// Saves the clear logo of a game as `<out_dir>/<slug of the name>.png` and returns the path.
/// `fetch` downloads a URL (`net::fetch`, replaced in tests).
pub fn get(
    index: &Index,
    request: &Request,
    fetch: impl FnOnce(&str) -> anyhow::Result<Vec<u8>>,
) -> anyhow::Result<PathBuf> {
    let Some(game) = index.game(request.id) else {
        bail!("there is no game with ID {}", request.id);
    };
    let logos = index
        .logos
        .get(&game.id)
        .map(Vec::as_slice)
        .unwrap_or_default();
    let Some(logo) = pick_logo(logos, request.region) else {
        bail!(
            "{} ({}) has no clear logo on LaunchBox",
            game.name,
            game.platform
        );
    };
    eprintln!(
        "{} ({}): clear logo, {}",
        game.name,
        game.platform,
        region_name(&logo.region)
    );
    let mut others: Vec<&str> = logos
        .iter()
        .map(|l| region_name(&l.region))
        .filter(|r| *r != region_name(&logo.region))
        .collect();
    others.sort_unstable();
    others.dedup();
    if !others.is_empty() {
        eprintln!("  also available (--region): {}", others.join(", "));
    }

    let name = if request.name.is_empty() {
        &game.name
    } else {
        request.name
    };
    let stem = slug(name);
    if stem.is_empty() {
        bail!("'{name}' gives an empty file name, use --name");
    }
    let png_target = request.out_dir.join(format!("{stem}.png"));
    if !request.force && png_target.exists() {
        bail!(
            "{} already exists, use --force to replace it",
            png_target.display()
        );
    }

    let data = fetch(&format!("{IMAGE_URL}{}", logo.file))?;
    let (data, target) = match to_png(&data) {
        Ok(png) => (png, png_target),
        Err(e) => {
            // Keep what LaunchBox has rather than nothing.
            let extension = Path::new(&logo.file)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("img")
                .to_lowercase();
            eprintln!("  cannot convert it to PNG ({e}), keeping it as .{extension}");
            (data, request.out_dir.join(format!("{stem}.{extension}")))
        }
    };
    if !request.force && target.exists() {
        bail!(
            "{} already exists, use --force to replace it",
            target.display()
        );
    }
    fs::create_dir_all(request.out_dir)
        .with_context(|| format!("cannot create {}", request.out_dir.display()))?;
    fs::write(&target, data).with_context(|| format!("cannot write {}", target.display()))?;
    Ok(target)
}

fn region_name(region: &str) -> &str {
    if region.is_empty() {
        "no region"
    } else {
        region
    }
}

/// PNG as it is; other formats (JPEG, WebP) are converted.
fn to_png(data: &[u8]) -> image::ImageResult<Vec<u8>> {
    if data.starts_with(PNG_MAGIC) {
        return Ok(data.to_vec());
    }
    let image = image::load_from_memory(data)?;
    let mut png = Cursor::new(Vec::new());
    image.write_to(&mut png, image::ImageFormat::Png)?;
    Ok(png.into_inner())
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;
    use crate::index::tests::fixture;

    struct Setup {
        index: Index,
        dir: PathBuf,
    }

    impl Setup {
        fn new(test: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "launchbox-images-test-{}-{test}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&dir);
            Setup {
                index: fixture(),
                dir,
            }
        }

        /// Runs `get` with a fake download returning `data`; returns the result and the URLs.
        fn get(
            &self,
            id: u64,
            region: &str,
            name: &str,
            force: bool,
            data: &[u8],
        ) -> (anyhow::Result<PathBuf>, Vec<String>) {
            let urls = RefCell::new(Vec::new());
            let request = Request {
                id,
                out_dir: &self.dir,
                region,
                name,
                force,
            };
            let result = get(&self.index, &request, |url| {
                urls.borrow_mut().push(url.to_string());
                Ok(data.to_vec())
            });
            (result, urls.into_inner())
        }
    }

    impl Drop for Setup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    fn png() -> Vec<u8> {
        encode(image::ImageFormat::Png)
    }

    fn encode(format: image::ImageFormat) -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(2, 2)
            .write_to(&mut out, format)
            .unwrap();
        out.into_inner()
    }

    #[test]
    fn saves_png_under_the_game_name() {
        let setup = Setup::new("png");
        let (result, urls) = setup.get(2, "", "", false, &png());
        assert_eq!(urls, [format!("{IMAGE_URL}titan.png")]);
        let target = result.unwrap();
        assert_eq!(target, setup.dir.join("time-crisis-project-titan.png"));
        assert_eq!(fs::read(target).unwrap(), png());
    }

    #[test]
    fn region_and_name() {
        let setup = Setup::new("region");
        let (result, urls) = setup.get(1, "Japan", "TC (JP)", false, &png());
        assert_eq!(urls, [format!("{IMAGE_URL}tc-japan.png")]);
        assert_eq!(result.unwrap(), setup.dir.join("tc-jp.png"));
    }

    #[test]
    fn keeps_existing_files() {
        let setup = Setup::new("existing");
        setup.get(1, "", "", false, &png()).0.unwrap();
        let (result, urls) = setup.get(1, "", "", false, &png());
        assert!(result.is_err());
        assert!(urls.is_empty(), "no download for an existing file");
        setup.get(1, "", "", true, &png()).0.unwrap();
    }

    #[test]
    fn converts_jpeg_to_png() {
        let setup = Setup::new("jpeg");
        let (result, _) = setup.get(3, "", "", false, &encode(image::ImageFormat::Jpeg));
        let target = result.unwrap();
        assert_eq!(target, setup.dir.join("point-blank.png"));
        assert!(fs::read(target).unwrap().starts_with(PNG_MAGIC));
    }

    #[test]
    fn keeps_unreadable_images_as_they_are() {
        let setup = Setup::new("unreadable");
        let (result, _) = setup.get(3, "", "", false, b"not an image");
        let target = result.unwrap();
        assert_eq!(target, setup.dir.join("point-blank.jpg"));
        assert_eq!(fs::read(target).unwrap(), b"not an image");
    }

    #[test]
    fn errors() {
        let setup = Setup::new("errors");
        assert!(
            setup.get(99, "", "", false, &png()).0.is_err(),
            "unknown ID"
        );
        assert!(
            setup.get(4, "", "", false, &png()).0.is_err(),
            "no clear logo"
        );
        assert!(
            setup.get(1, "", "!?", false, &png()).0.is_err(),
            "empty name"
        );
    }
}
