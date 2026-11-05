//! Title alias and localization resolution.

fn is_stop_word(w: &str) -> bool {
    matches!(
        w,
        "the"
            | "a"
            | "an"
            | "on"
            | "no"
            | "of"
            | "in"
            | "to"
            | "and"
            | "or"
            | "for"
            | "with"
            | "season"
            | "part"
    )
}

/// Derive an English display title for known Romaji anime titles.
#[must_use]
pub fn english_display_title(title: &str) -> Option<String> {
    let lower = title.to_ascii_lowercase();
    if lower.contains("shingeki no kyojin") {
        Some(title.replace("Shingeki no Kyojin", "Attack on Titan"))
    } else if lower.contains("kimetsu no yaiba") {
        Some(title.replace("Kimetsu no Yaiba", "Demon Slayer"))
    } else if lower.contains("boku no hero academia") {
        Some(title.replace("Boku no Hero Academia", "My Hero Academia"))
    } else if lower.contains("gotoubun no hanayome")
        || lower.contains("go-toubun no hanayome")
        || lower.contains("5-toubun no hanayome")
    {
        Some("The Quintessential Quintuplets".to_string())
    } else if lower.contains("sousou no frieren") {
        Some(title.replace("Sousou no Frieren", "Frieren: Beyond Journey's End"))
    } else if lower.contains("tensei shitara slime datta ken") {
        Some(title.replace(
            "Tensei shitara Slime Datta Ken",
            "That Time I Got Reincarnated as a Slime",
        ))
    } else {
        None
    }
}

/// Static alias dictionary mapping search patterns to search targets.
const ALIAS_DICTIONARY: &[(&[&str], &[&str])] = &[
    (
        &[
            "the quintessential quintuplets",
            "quintessential quintuplets",
            "quintuplets",
            "quintessential",
            "gotoubun",
            "go-toubun",
            "5-toubun",
        ],
        &[
            "Go-toubun no Hanayome",
            "Gotoubun no Hanayome",
            "5-toubun no Hanayome",
        ],
    ),
    (
        &[
            "attack on titan",
            "attack",
            "titan",
            "aot",
            "snk",
            "shingeki",
            "shingeki no kyojin",
        ],
        &["Shingeki no Kyojin", "Shingeki", "Attack on Titan"],
    ),
    (
        &["solo leveling", "solo", "ore dake level up na ken"],
        &["Solo Leveling", "Ore dake Level Up na Ken"],
    ),
    (
        &[
            "demon slayer",
            "demon",
            "slayer",
            "kimetsu",
            "kimetsu no yaiba",
            "yaiba",
        ],
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
            "hero academia",
            "mha",
            "heroaca",
            "boku no hero",
        ],
        &["Boku no Hero Academia", "My Hero Academia"],
    ),
    (&["jujutsu kaisen", "jujutsu", "jjk"], &["Jujutsu Kaisen"]),
    (
        &["spy x family", "spyxfamily", "spy family", "spy"],
        &["SPY x FAMILY", "Spy x Family"],
    ),
    (&["chainsaw man", "chainsaw", "csm"], &["Chainsaw Man"]),
    (
        &["oshi no ko", "oshi", "my star"],
        &["[Oshi No Ko]", "Oshi no Ko"],
    ),
    (
        &[
            "that time i got reincarnated as a slime",
            "slime",
            "tensura",
            "tensei shitara slime datta ken",
        ],
        &["Tensei shitara Slime Datta Ken", "Tensura Nikki"],
    ),
    (
        &["konosuba", "kono subarashii"],
        &["Kono Subarashii Sekai ni Shukufuku wo!"],
    ),
    (
        &["re:zero", "rezero", "starting life in another world"],
        &["Re:Zero kara Hajimeru Isekai Seikatsu", "Re:Zero"],
    ),
    (
        &["mushoku tensei", "mushoku", "jobless reincarnation"],
        &["Mushoku Tensei"],
    ),
    (&["bleach", "tybw", "thousand-year blood war"], &["Bleach"]),
    (
        &[
            "danmachi",
            "is it wrong to try to pick up girls in a dungeon",
            "dungeon ni deai",
        ],
        &["Dungeon ni Deai wo Motomeru no wa Machigatteiru Darou ka"],
    ),
    (
        &["classroom of the elite", "cote", "youkoso jitsuryoku"],
        &["Youkoso Jitsuryoku Shijou Shugi no Kyoushitsu e"],
    ),
    (
        &["kaguya-sama", "kaguya", "love is war"],
        &["Kaguya-sama wa Kokurasetai"],
    ),
    (&["bocchi the rock", "bocchi", "btr"], &["Bocchi the Rock!"]),
    (&["blue lock"], &["Blue Lock"]),
    (
        &["kaiju no. 8", "kaiju no 8", "kaiju 8", "kaiju"],
        &["Kaiju No. 8"],
    ),
    (&["vinland saga", "vinland"], &["Vinland Saga"]),
    (&["dandadan"], &["Dandadan"]),
];

/// Resolve alternative English, Romaji, and common names for an anime title.
#[must_use]
pub fn resolve_anime_aliases(input: &str) -> Vec<String> {
    let trimmed = input.trim();
    let lower = trimmed.to_ascii_lowercase();

    let mut results = vec![trimmed.to_string()];

    let query_words: Vec<&str> = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();

    for (patterns, targets) in ALIAS_DICTIONARY {
        let matched = patterns.iter().any(|&pattern| {
            if lower == pattern || lower.contains(pattern) {
                return true;
            }
            if lower.len() >= 3 && !is_stop_word(&lower) && pattern.contains(&lower) {
                return true;
            }
            query_words.iter().any(|&qw| {
                qw.len() >= 3
                    && !is_stop_word(qw)
                    && pattern
                        .split(|c: char| !c.is_alphanumeric())
                        .filter(|pw| !pw.is_empty())
                        .any(|pw| pw == qw)
            })
        });

        if matched {
            for target in *targets {
                if !results.iter().any(|r| r.eq_ignore_ascii_case(target)) {
                    results.push((*target).to_string());
                }
            }
        }
    }

    // Also add variations without "The " or punctuation
    if lower.starts_with("the ") {
        let stripped = trimmed[4..].trim().to_string();
        if !results.iter().any(|r| r.eq_ignore_ascii_case(&stripped)) {
            results.push(stripped);
        }
    }

    results
}
