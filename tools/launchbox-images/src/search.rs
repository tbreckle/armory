//! Searching the index, file names and the choice between several logos of a game.

use unicode_normalization::UnicodeNormalization;

use crate::index::{Game, Index, Logo};

/// Logos without a region come first, then the most common English ones.
const REGION_ORDER: [&str; 6] = [
    "",
    "World",
    "North America",
    "United States",
    "Europe",
    "United Kingdom",
];

/// Lowercase ASCII of a text: accents dropped ("é" → "e"), other non-ASCII characters removed.
fn ascii_lower(text: &str) -> String {
    text.nfkd()
        .filter(char::is_ascii)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// Words of a text for searching: lowercase a-z/0-9 runs without accents and punctuation.
fn words(text: &str) -> Vec<String> {
    ascii_lower(text)
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_owned)
        .collect()
}

/// File name for a game: lowercase, every run of other characters than a-z/0-9 becomes '-'.
///
/// Accents are dropped first, so "Pokémon" becomes "pokemon" instead of "pok-mon".
pub fn slug(name: &str) -> String {
    words(name).join("-")
}

/// Games whose name or alternate name contains all words of the query (each as the start of a
/// word, in any order), best matches first: exact name, name starting with the first word, rest.
/// `platform` keeps only platforms containing it; `with_logo` only games with a clear logo.
pub fn search<'a>(
    index: &'a Index,
    query: &str,
    platform: &str,
    with_logo: bool,
) -> anyhow::Result<Vec<&'a Game>> {
    let query = words(query);
    if query.is_empty() {
        anyhow::bail!("the search needs at least one letter or digit");
    }
    let wanted_platform = words(platform).join(" ");
    let joined = query.join(" ");

    let mut hits: Vec<(u8, String, &Game)> = Vec::new();
    for game in &index.games {
        if with_logo && !index.logos.contains_key(&game.id) {
            continue;
        }
        if !wanted_platform.is_empty()
            && !words(&game.platform).join(" ").contains(&wanted_platform)
        {
            continue;
        }
        let alternates = index.alternates.get(&game.id).into_iter().flatten();
        let rank = std::iter::once(&game.name)
            .chain(alternates)
            .map(|name| words(name))
            .filter(|name| query.iter().all(|q| name.iter().any(|w| w.starts_with(q))))
            .map(|name| {
                let name = name.join(" ");
                if name == joined {
                    0
                } else if name.starts_with(&query[0]) {
                    1
                } else {
                    2
                }
            })
            .min();
        if let Some(rank) = rank {
            hits.push((rank, game.name.to_lowercase(), game));
        }
    }
    hits.sort_by(|a, b| {
        (a.0, &a.1, &a.2.platform, a.2.id).cmp(&(b.0, &b.1, &b.2.platform, b.2.id))
    });
    Ok(hits.into_iter().map(|(_, _, game)| game).collect())
}

/// The preferred logo: the asked region, then `REGION_ORDER`, then the first other one.
/// `None` only for an empty list.
pub fn pick_logo<'a>(logos: &'a [Logo], region: &str) -> Option<&'a Logo> {
    let order: Vec<&str> = (!region.is_empty())
        .then_some(region)
        .into_iter()
        .chain(REGION_ORDER)
        .collect();
    logos.iter().min_by_key(|logo| {
        order
            .iter()
            .position(|wanted| logo.region.eq_ignore_ascii_case(wanted))
            .unwrap_or(order.len())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::tests::fixture;

    #[test]
    fn slug_lowercase_and_dashes() {
        assert_eq!(
            slug("Time Crisis: Project Titan"),
            "time-crisis-project-titan"
        );
        assert_eq!(
            slug("House of the Dead 2 & 3 Return"),
            "house-of-the-dead-2-3-return"
        );
        assert_eq!(slug("  -Area 51-  "), "area-51");
    }

    #[test]
    fn slug_drops_accents() {
        assert_eq!(slug("Pokémon Snap"), "pokemon-snap");
    }

    #[test]
    fn slug_of_only_special_characters_is_empty() {
        assert_eq!(slug("ポイントブランク"), "");
    }

    fn ids(query: &str, platform: &str, with_logo: bool) -> Vec<u64> {
        let index = fixture();
        search(&index, query, platform, with_logo)
            .unwrap()
            .iter()
            .map(|g| g.id)
            .collect()
    }

    #[test]
    fn exact_match_first() {
        assert_eq!(ids("time crisis", "", true), [1, 2]);
    }

    #[test]
    fn words_in_any_order_and_prefixes() {
        assert_eq!(ids("titan TIME", "", true), [2]);
        assert_eq!(ids("tim cri", "", true), [1, 2]);
    }

    #[test]
    fn punctuation_is_ignored() {
        assert_eq!(ids("Time-Crisis: Project", "", true), [2]);
    }

    #[test]
    fn matches_word_starts_only() {
        assert_eq!(ids("risis", "", true), Vec::<u64>::new());
    }

    #[test]
    fn searches_alternate_names() {
        assert_eq!(ids("gun bullet", "", true), [3]);
    }

    #[test]
    fn filters_platform() {
        assert_eq!(ids("crisis", "playstation", true), [2]);
    }

    #[test]
    fn games_without_logo_on_request() {
        assert_eq!(ids("crisis", "", true), [1, 2]);
        assert_eq!(ids("crisis", "", false), [4, 1, 2]);
    }

    #[test]
    fn empty_query_is_an_error() {
        assert!(search(&fixture(), "?!", "", true).is_err());
    }

    fn logos(list: &[(&str, &str)]) -> Vec<Logo> {
        list.iter()
            .map(|(file, region)| Logo {
                file: file.to_string(),
                region: region.to_string(),
            })
            .collect()
    }

    #[test]
    fn logo_default_order() {
        let list = logos(&[
            ("jp.png", "Japan"),
            ("eu.png", "Europe"),
            ("na.png", "North America"),
        ]);
        assert_eq!(pick_logo(&list, "").unwrap().file, "na.png");
        let mut with_none = list.clone();
        with_none.push(logos(&[("none.png", "")]).remove(0));
        assert_eq!(pick_logo(&with_none, "").unwrap().file, "none.png");
        assert_eq!(pick_logo(&list, "japan").unwrap().file, "jp.png");
    }

    #[test]
    fn logo_unknown_regions_keep_order() {
        let list = logos(&[("a.png", "Brazil"), ("b.png", "Korea")]);
        assert_eq!(pick_logo(&list, "").unwrap().file, "a.png");
        assert!(pick_logo(&[], "").is_none());
    }
}
