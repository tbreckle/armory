//! A.R.M.O.R.Y. (Arcade Runner Menu On Rails, Yeah!)

/// Version injected by build.rs (`ARMORY_VERSION`, see scripts/version.sh).
const VERSION: &str = env!("ARMORY_VERSION");

fn main() {
    println!("{}", banner());
}

fn banner() -> String {
    format!("A.R.M.O.R.Y. {VERSION}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banner_contains_version() {
        assert!(banner().ends_with(VERSION));
    }
}
