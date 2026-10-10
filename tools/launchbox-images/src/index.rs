//! The local index: games, alternate names and clear logos out of LaunchBox's Metadata.zip,
//! stored gzipped as JSON in the cache directory.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read};
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use quick_xml::events::Event;
use serde::{Deserialize, Serialize};

use crate::net;

const METADATA_URL: &str = "https://gamesdb.launchbox-app.com/Metadata.zip";
const INDEX_NAME: &str = "index.json.gz";
/// Bump when `Index` changes; an older index is then rebuilt.
const INDEX_VERSION: u32 = 1;
const LOGO_TYPE: &str = "Clear Logo";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Game {
    pub id: u64,
    pub name: String,
    pub platform: String,
    /// Release year, empty when unknown.
    pub year: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Logo {
    /// File name on images.launchbox-app.com.
    pub file: String,
    /// Empty when the logo has no region.
    pub region: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Index {
    pub version: u32,
    pub games: Vec<Game>,
    pub alternates: HashMap<u64, Vec<String>>,
    pub logos: HashMap<u64, Vec<Logo>>,
}

impl Index {
    pub fn game(&self, id: u64) -> Option<&Game> {
        self.games.iter().find(|g| g.id == id)
    }
}

/// Default cache directory: armory/launchbox in the user's cache directory.
pub fn default_cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("armory")
        .join("launchbox")
}

/// The cached index; downloads the database on first use or when the index is unusable.
pub fn load(cache: &Path) -> anyhow::Result<Index> {
    let path = cache.join(INDEX_NAME);
    match read(&path) {
        Ok(index) if index.version == INDEX_VERSION => return Ok(index),
        Ok(_) => eprintln!(
            "The index in {} is outdated, rebuilding it.",
            cache.display()
        ),
        Err(e) if path.exists() => eprintln!(
            "The index in {} is unreadable ({e:#}), downloading again.",
            cache.display()
        ),
        Err(_) => {}
    }
    update(cache)
}

fn read(path: &Path) -> anyhow::Result<Index> {
    let file = File::open(path)?;
    Ok(serde_json::from_reader(BufReader::new(GzDecoder::new(
        BufReader::new(file),
    )))?)
}

/// Downloads Metadata.zip, builds the index, stores it and removes the zip again.
pub fn update(cache: &Path) -> anyhow::Result<Index> {
    fs::create_dir_all(cache).with_context(|| format!("cannot create {}", cache.display()))?;
    let zip_path = cache.join("Metadata.zip");
    eprintln!("Downloading {METADATA_URL} ...");
    net::download(METADATA_URL, &zip_path)?;
    eprintln!("Building the index ...");
    let index = File::open(&zip_path)
        .map_err(anyhow::Error::from)
        .and_then(|file| build(BufReader::new(file)))
        .with_context(|| format!("cannot read {}", zip_path.display()));
    // The zip is only needed for building; a failed build downloads it again anyway.
    let _ = fs::remove_file(&zip_path);
    let index = index?;

    let path = cache.join(INDEX_NAME);
    let tmp = cache.join(format!("{INDEX_NAME}.tmp"));
    let write = || -> anyhow::Result<()> {
        let mut out = GzEncoder::new(BufWriter::new(File::create(&tmp)?), Compression::default());
        serde_json::to_writer(&mut out, &index)?;
        out.finish()?.into_inner().map_err(|e| e.into_error())?;
        fs::rename(&tmp, &path)?;
        Ok(())
    };
    write().with_context(|| format!("cannot write {}", path.display()))?;
    eprintln!(
        "{} games, {} with a clear logo.",
        index.games.len(),
        index.logos.len()
    );
    Ok(index)
}

/// Reads the games, alternate names and clear logos out of a Metadata.zip.
pub fn build(zip: impl Read + std::io::Seek) -> anyhow::Result<Index> {
    let mut archive = zip::ZipArchive::new(zip)?;
    let xml = archive
        .by_name("Metadata.xml")
        .context("Metadata.xml is missing")?;
    let index = parse(BufReader::with_capacity(1 << 16, xml))?;
    if index.games.is_empty() {
        bail!("no games found, is it a LaunchBox Metadata.zip?");
    }
    Ok(index)
}

/// Parses Metadata.xml as a stream (it is about 500 MB): only the direct children of `<Game>`,
/// `<GameAlternateName>` and `<GameImage>` that the index needs are kept.
fn parse(xml: impl std::io::BufRead) -> anyhow::Result<Index> {
    let mut index = Index {
        version: INDEX_VERSION,
        ..Index::default()
    };
    let mut reader = quick_xml::Reader::from_reader(xml);
    let mut buf = Vec::new();
    // Element names from the root down, and the fields of the current record.
    let mut path: Vec<String> = Vec::new();
    let mut fields: HashMap<&'static str, String> = HashMap::new();
    let mut field: Option<&'static str> = None;

    loop {
        let event = reader
            .read_event_into(&mut buf)
            .with_context(|| format!("XML error at byte {}", reader.buffer_position()))?;
        match event {
            Event::Start(e) => {
                path.push(e.name().as_ref().to_owned());
                field = if path.len() == 3 && is_record(&path[1]) {
                    wanted_field(e.name().as_ref())
                } else {
                    None
                };
            }
            Event::End(_) => {
                if path.len() == 2 {
                    add_record(&mut index, &path[1], &mut fields);
                }
                path.pop();
                field = None;
            }
            Event::Text(text) => {
                if let Some(name) = field {
                    fields
                        .entry(name)
                        .or_default()
                        .push_str(&text.xml10_content());
                }
            }
            Event::CData(data) => {
                if let Some(name) = field {
                    fields.entry(name).or_default().push_str(&data);
                }
            }
            Event::GeneralRef(entity) => {
                if let Some(name) = field {
                    let c = match entity.resolve_char_ref()? {
                        Some(c) => Some(c),
                        None => match entity.as_ref() {
                            "amp" => Some('&'),
                            "lt" => Some('<'),
                            "gt" => Some('>'),
                            "quot" => Some('"'),
                            "apos" => Some('\''),
                            _ => None,
                        },
                    };
                    if let Some(c) = c {
                        fields.entry(name).or_default().push(c);
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(index)
}

fn is_record(name: &str) -> bool {
    matches!(name, "Game" | "GameAlternateName" | "GameImage")
}

fn wanted_field(name: &str) -> Option<&'static str> {
    Some(match name {
        "DatabaseID" => "id",
        "Name" => "name",
        "Platform" => "platform",
        "ReleaseDate" => "date",
        "ReleaseYear" => "year",
        "AlternateName" => "alternate",
        "FileName" => "file",
        "Type" => "type",
        "Region" => "region",
        _ => return None,
    })
}

fn add_record(index: &mut Index, record: &str, fields: &mut HashMap<&'static str, String>) {
    let mut take = |key| {
        fields
            .remove(key)
            .map(|v| v.trim().to_owned())
            .unwrap_or_default()
    };
    let id = take("id").parse::<u64>().ok();
    match (record, id) {
        ("Game", Some(id)) => {
            let name = take("name");
            let date = Some(take("date"))
                .filter(|d| !d.is_empty())
                .unwrap_or_else(|| take("year"));
            if !name.is_empty() {
                index.games.push(Game {
                    id,
                    name,
                    platform: take("platform"),
                    year: date.chars().take(4).collect(),
                });
            }
        }
        ("GameAlternateName", Some(id)) => {
            let name = take("alternate");
            if !name.is_empty() {
                index.alternates.entry(id).or_default().push(name);
            }
        }
        ("GameImage", Some(id)) if take("type") == LOGO_TYPE => {
            let file = take("file");
            if !file.is_empty() {
                let region = take("region");
                index
                    .logos
                    .entry(id)
                    .or_default()
                    .push(Logo { file, region });
            }
        }
        _ => {}
    }
    fields.clear();
}

#[cfg(test)]
pub(crate) mod tests {
    use std::io::{Cursor, Write};

    use super::*;

    const METADATA: &str = r#"<?xml version="1.0" standalone="yes"?>
<LaunchBox>
  <Game>
    <Name>Time Crisis</Name>
    <ReleaseDate>1995-12-01T00:00:00-08:00</ReleaseDate>
    <Overview>Not <i>indexed</i>.</Overview>
    <DatabaseID>1</DatabaseID>
    <Platform>Arcade</Platform>
    <Genres />
  </Game>
  <Game>
    <Name>Time Crisis: Project Titan</Name>
    <ReleaseYear>2001</ReleaseYear>
    <DatabaseID>2</DatabaseID>
    <Platform>Sony Playstation</Platform>
  </Game>
  <Game>
    <Name>Point Blank</Name>
    <DatabaseID>3</DatabaseID>
    <Platform>Arcade</Platform>
  </Game>
  <Game>
    <Name>Crisis Zone</Name>
    <DatabaseID>4</DatabaseID>
    <Platform>Arcade</Platform>
  </Game>
  <Game>
    <Name>Tom &amp; Jerry &#x2014; &lt;Demo&gt;</Name>
    <DatabaseID>5</DatabaseID>
    <Platform>Nintendo Entertainment System</Platform>
  </Game>
  <Game>
    <Name>No ID</Name>
  </Game>
  <GameAlternateName>
    <AlternateName>Gun Bullet</AlternateName>
    <DatabaseID>3</DatabaseID>
    <Region>Japan</Region>
  </GameAlternateName>
  <GameImage>
    <DatabaseID>1</DatabaseID>
    <FileName>tc-japan.png</FileName>
    <Type>Clear Logo</Type>
    <Region>Japan</Region>
  </GameImage>
  <GameImage>
    <DatabaseID>1</DatabaseID>
    <FileName>tc.png</FileName>
    <Type>Clear Logo</Type>
  </GameImage>
  <GameImage>
    <DatabaseID>1</DatabaseID>
    <FileName>tc-box.jpg</FileName>
    <Type>Box - Front</Type>
  </GameImage>
  <GameImage>
    <DatabaseID>2</DatabaseID>
    <FileName>titan.png</FileName>
    <Type>Clear Logo</Type>
    <Region>North America</Region>
  </GameImage>
  <GameImage>
    <DatabaseID>3</DatabaseID>
    <FileName>pb.jpg</FileName>
    <Type>Clear Logo</Type>
  </GameImage>
</LaunchBox>
"#;

    fn zip_of(name: &str, content: &str) -> Cursor<Vec<u8>> {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(content.as_bytes()).unwrap();
        let mut cursor = zip.finish().unwrap();
        cursor.set_position(0);
        cursor
    }

    pub(crate) fn fixture() -> Index {
        build(zip_of("Metadata.xml", METADATA)).unwrap()
    }

    fn game(id: u64, name: &str, platform: &str, year: &str) -> Game {
        Game {
            id,
            name: name.into(),
            platform: platform.into(),
            year: year.into(),
        }
    }

    #[test]
    fn reads_games() {
        assert_eq!(
            fixture().games,
            [
                game(1, "Time Crisis", "Arcade", "1995"),
                game(2, "Time Crisis: Project Titan", "Sony Playstation", "2001"),
                game(3, "Point Blank", "Arcade", ""),
                game(4, "Crisis Zone", "Arcade", ""),
                game(
                    5,
                    "Tom & Jerry \u{2014} <Demo>",
                    "Nintendo Entertainment System",
                    ""
                ),
            ]
        );
    }

    #[test]
    fn reads_only_clear_logos() {
        let index = fixture();
        let files: Vec<(&str, &str)> = index.logos[&1]
            .iter()
            .map(|l| (l.file.as_str(), l.region.as_str()))
            .collect();
        assert_eq!(files, [("tc-japan.png", "Japan"), ("tc.png", "")]);
        assert_eq!(index.logos.len(), 3);
    }

    #[test]
    fn reads_alternate_names() {
        assert_eq!(
            fixture().alternates,
            HashMap::from([(3, vec!["Gun Bullet".into()])])
        );
    }

    #[test]
    fn rejects_other_archives() {
        assert!(build(zip_of("Metadata.xml", "<LaunchBox></LaunchBox>")).is_err());
        assert!(build(zip_of("Other.xml", METADATA)).is_err());
        assert!(build(Cursor::new(b"not a zip".to_vec())).is_err());
    }

    #[test]
    fn index_survives_json() {
        let index = fixture();
        let back: Index = serde_json::from_str(&serde_json::to_string(&index).unwrap()).unwrap();
        assert_eq!(back.games, index.games);
        assert_eq!(back.logos, index.logos);
    }
}
