const NARRATIVE_TAGS: &[&str] = &[
    "brief",
    "summary",
    "short",
    "details",
    "description",
    "invariant",
    "note",
    "warning",
    "attention",
    "remark",
    "remarks",
    "pre",
    "post",
    "deprecated",
    "todo",
];

const BLOCK_TAGS: &[&str] = &[
    "param",
    "tparam",
    "returns",
    "return",
    "retval",
    "throws",
    "throw",
    "exception",
    "sa",
    "see",
    "since",
    "note",
    "warning",
    "remarks",
    "remark",
    "pre",
    "post",
    "invariant",
    "complexity",
    "requires",
    "ensures",
    "example",
    "liveexample",
    "code",
    "endcode",
    "ingroup",
    "defgroup",
    "file",
    "author",
    "version",
    "copyright",
    "deprecated",
    "todo",
    "attention",
    "cond",
    "endcond",
    "relatesalso",
    "typeparam",
    "value",
    "inheritdoc",
];

fn tag_at(text: &str, at: usize) -> Option<&'static str> {
    let rest = &text[at..];
    let marker = rest.chars().next()?;
    if marker != '@' && marker != '\\' {
        return None;
    }
    let name: String = rest[1..].chars().take_while(|glyph| glyph.is_ascii_alphabetic()).collect();
    if name.is_empty() {
        return None;
    }
    let lowered = name.to_ascii_lowercase();
    BLOCK_TAGS.iter().chain(NARRATIVE_TAGS.iter()).find(|known| **known == lowered).copied()
}

fn starts_a_word(text: &str, at: usize) -> bool {
    at == 0 || text[..at].ends_with(char::is_whitespace)
}

pub(crate) fn unwrap_summary(text: &str) -> String {
    let trimmed = text.trim_start();
    let Some(name) = tag_at(trimmed, 0) else {
        return text.to_owned();
    };
    if !NARRATIVE_TAGS.contains(&name) {
        return text.to_owned();
    }
    trimmed[1 + name.len()..].trim_start().to_owned()
}

pub(crate) fn break_before_first_block_tag(text: &str) -> String {
    let start = unwrap_summary(text);
    let boundary =
        start.char_indices().filter(|(at, _)| *at > 0 && starts_a_word(&start, *at)).find_map(
            |(at, _)| tag_at(&start, at).filter(|name| BLOCK_TAGS.contains(name)).map(|_| at),
        );

    let Some(at) = boundary else {
        return start;
    };
    let head = start[..at].trim_end();
    let tail = start[at..].trim();
    if head.is_empty() || tail.is_empty() {
        return start;
    }
    format!("{head}\n\n{tail}")
}

#[cfg(test)]
mod tests {
    use super::{break_before_first_block_tag, unwrap_summary};

    #[test]
    fn a_summary_tag_is_not_part_of_the_claim() {
        assert_eq!(unwrap_summary("@brief a type for a number"), "a type for a number");
        assert_eq!(unwrap_summary("\\brief a type for a number"), "a type for a number");
    }

    #[test]
    fn a_block_tag_starts_the_detail() {
        assert_eq!(
            break_before_first_block_tag("@brief a number type @sa https://example.invalid/"),
            "a number type\n\n@sa https://example.invalid/"
        );
    }

    #[test]
    fn javadoc_reads_the_same_way() {
        assert_eq!(
            break_before_first_block_tag(
                "Parses the header. @param raw the bytes @return a header"
            ),
            "Parses the header.\n\n@param raw the bytes @return a header"
        );
    }

    #[test]
    fn an_address_in_prose_is_not_a_tag() {
        let given = "Send to team@example.invalid before the release.";
        assert_eq!(break_before_first_block_tag(given), given);
    }

    #[test]
    fn an_unknown_tag_is_left_alone() {
        let given = "@madeuptag something nobody documents with";
        assert_eq!(break_before_first_block_tag(given), given);
    }

    #[test]
    fn a_leading_narrative_tag_is_a_marker_and_its_content_is_the_claim() {
        assert_eq!(
            break_before_first_block_tag("@invariant If the ref stack is empty the value is root."),
            "If the ref stack is empty the value is root."
        );
    }

    #[test]
    fn the_same_tag_later_in_the_text_starts_the_detail_instead() {
        assert_eq!(
            break_before_first_block_tag("Parses a value. @note Empty input is an error."),
            "Parses a value.

@note Empty input is an error."
        );
    }

    #[test]
    fn prose_without_tags_is_untouched() {
        let given = "Validation must precede tenant resolution.";
        assert_eq!(break_before_first_block_tag(given), given);
    }
}
