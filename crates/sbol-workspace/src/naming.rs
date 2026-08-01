//! Stable filesystem-safe collection names.

use url::Url;

/// Derive a succinct filesystem-safe name from an advertised display id or a
/// versioned collection URI. The URI version is never used as the filename.
pub fn safe_collection_name(uri: &str, display_id: Option<&str>) -> String {
    let source = display_id
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
        .or_else(|| {
            Url::parse(uri).ok().and_then(|url| {
                let segments = url.path_segments()?.collect::<Vec<_>>();
                let candidate = if segments.len() >= 2 {
                    segments[segments.len() - 2]
                } else {
                    segments.last().copied().unwrap_or("design")
                };
                Some(candidate.trim_end_matches("_collection").to_owned())
            })
        })
        .unwrap_or_else(|| "design".to_owned());
    let mut output = String::new();
    let mut separator = false;
    for character in source.chars() {
        if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
            output.push(character);
            separator = false;
        } else if !separator && !output.is_empty() {
            output.push('_');
            separator = true;
        }
    }
    while output.ends_with(['_', '-']) {
        output.pop();
    }
    if output.is_empty() {
        "design".to_owned()
    } else {
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collection_name_uses_display_or_identity_not_version() {
        assert_eq!(
            safe_collection_name(
                "https://sbol.io/user/alice/toggle/toggle_collection/1",
                None
            ),
            "toggle"
        );
        assert_eq!(
            safe_collection_name("https://sbol.io/design/99", Some("pTet toggle")),
            "pTet_toggle"
        );
    }
}
