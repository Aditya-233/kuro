//! Title alias and localization resolution.

/// Resolve alternative English, Romaji, and common names for an anime title.
#[must_use]
pub fn resolve_anime_aliases(input: &str) -> Vec<String> {
    let trimmed = input.trim();
    let lower = trimmed.to_ascii_lowercase();

    let mut results = vec![trimmed.to_string()];

    // Common alias dictionary
    let dictionary: &[(&[&str], &[&str])] = &[
        (
            &[
                "the quintessential quintuplets",
                "quintessential quintuplets",
                "gotoubun",
                "go-toubun",
            ],
            &[
                "Go-toubun no Hanayome",
                "Gotoubun no Hanayome",
                "5-toubun no Hanayome",
            ],
        ),
        (
            &["attack on titan", "aot", "shingeki"],
            &["Shingeki no Kyojin", "Attack on Titan"],
        ),
        (
            &["solo leveling", "ore dake level up na ken"],
            &["Solo Leveling", "Ore dake Level Up na Ken"],
        ),
        (
            &["demon slayer", "kimetsu"],
            &["Kimetsu no Yaiba", "Demon Slayer"],
        ),
        (
            &[
                "frieren",
                "sousou no frieren",
                "frieren: beyond journey's end",
            ],
            &["Sousou no Frieren", "Frieren"],
        ),
        (
            &[
                "my hero academia",
                "boku no hero academia",
                "mha",
                "heroaca",
            ],
            &["Boku no Hero Academia", "My Hero Academia"],
        ),
        (&["jujutsu kaisen", "jjk"], &["Jujutsu Kaisen"]),
        (
            &["spy x family", "spyxfamily", "spy family"],
            &["SPY x FAMILY", "Spy x Family"],
        ),
        (&["chainsaw man", "csm"], &["Chainsaw Man"]),
        (&["oshi no ko", "my star"], &["[Oshi No Ko]", "Oshi no Ko"]),
    ];

    for (patterns, targets) in dictionary {
        if patterns.iter().any(|p| lower.contains(p)) {
            for target in *targets {
                if !results.iter().any(|r| r.eq_ignore_ascii_case(target)) {
                    results.push((*target).to_string());
                }
            }
        }
    }

    // Also add variations without "The " or punctuation
    if lower.starts_with("the ") {
        results.push(trimmed[4..].trim().to_string());
    }

    results
}
