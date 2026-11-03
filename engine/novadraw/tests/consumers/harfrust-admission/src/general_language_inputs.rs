//! Same ordered input corpus for original and patched ordinary libraries.
pub fn languages() -> Vec<String> {
    let source = include_str!("../../../../dependencies/harfrust-0.3.2/src/hb/tag_rules.rs");
    let mut first = None;
    let mut result = Vec::new();
    for line in source.lines() {
        if line.starts_with("fn subtag_with") {
            break;
        }
        if let Some(rest) = line.strip_prefix("const ") {
            let name = rest.split(':').next().unwrap();
            first = if name == "GLOBAL" {
                None
            } else {
                name.chars().next().map(|c| c.to_ascii_lowercase())
            };
        }
        let line = line.trim();
        if !line.contains("=> [b\"") {
            continue;
        }
        let condition = line.split("=>").next().unwrap();
        let strings: Vec<_> = condition.split('"').skip(1).step_by(2).collect();
        let input = if condition.starts_with("subtag ") {
            format!("und{}", strings[0])
        } else if condition.starts_with("both ") {
            format!("{}{}abc{}", first.unwrap(), strings[0], strings[1])
        } else {
            format!("{}{}", first.unwrap(), strings[0])
        };
        result.push(input);
    }
    assert_eq!(result.len(), 148);
    result.extend(
        [
            "en",
            "en-US",
            "und-abc",
            "und-zzzz",
            "zzzz",
            "x-hbot",
            "en-x-hbscdflt",
            "en-fonipaa-fonipa",
            "cmn-xxx-hka-hk",
            "zh-hant-moa",
            "zh-hant-mo-abc",
            "zh-hant-moa-mo",
            "é",
            "éé",
            "a",
            "zzj",
            "abc-abc",
            "und-fonipaé",
        ]
        .into_iter()
        .map(String::from),
    );
    result.push(format!("und-{}-fonipa", "a".repeat(1024)));
    result
}
