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

const INLINE_TAGS: &[&str] = &["link", "linkplain", "code", "literal", "value", "inheritdoc"];

const XMLDOC_BLOCK: &[&str] = &[
    "param",
    "returns",
    "remarks",
    "exception",
    "example",
    "seealso",
    "typeparam",
    "value",
    "permission",
    "returns",
];

const HTML_TAGS: &[&str] = &[
    "summary",
    "param",
    "returns",
    "remarks",
    "exception",
    "example",
    "seealso",
    "typeparam",
    "value",
    "permission",
    "paramref",
    "typeparamref",
    "see",
    "c",
    "list",
    "item",
    "term",
    "description",
    "inheritdoc",
    "para",
    "p",
    "br",
    "code",
    "pre",
    "ul",
    "ol",
    "li",
    "b",
    "i",
    "em",
    "strong",
    "a",
    "tt",
    "blockquote",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "table",
    "tr",
    "td",
    "th",
    "div",
    "span",
    "dl",
    "dt",
    "dd",
    "sup",
    "sub",
];

pub(crate) fn unwrap_inline_tags(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find("{@") {
        let name: String =
            rest[at + 2..].chars().take_while(|glyph| glyph.is_ascii_alphabetic()).collect();
        let Some(close) = rest[at..].find('}') else {
            break;
        };
        if name.is_empty() || !INLINE_TAGS.contains(&name.to_ascii_lowercase().as_str()) {
            out.push_str(&rest[..at + 2]);
            rest = &rest[at + 2..];
            continue;
        }
        out.push_str(&rest[..at]);
        let inner = rest[at + 2 + name.len()..at + close].trim();
        out.push_str(inner);
        rest = &rest[at + close + 1..];
    }
    out.push_str(rest);
    out
}

fn html_tag_at(text: &str, at: usize) -> Option<usize> {
    let rest = &text[at..];
    let close = rest.find('>')?;
    let inside = rest[1..close].trim_start_matches('/').trim();
    let name: String = inside.chars().take_while(|glyph| glyph.is_ascii_alphanumeric()).collect();
    if name.is_empty() {
        return None;
    }
    let tail = inside[name.len()..].trim_start();
    let attributes_only = tail.is_empty()
        || tail.starts_with(|glyph: char| glyph.is_ascii_alphabetic() || glyph == '/');
    if !attributes_only {
        return None;
    }
    HTML_TAGS.contains(&name.to_ascii_lowercase().as_str()).then_some(close + 1)
}

pub(crate) fn strip_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut at = 0usize;
    while at < text.len() {
        if text[at..].starts_with('<')
            && let Some(width) = html_tag_at(text, at)
        {
            let name: String = text[at + 1..at + width]
                .trim_start_matches('/')
                .chars()
                .take_while(|glyph| glyph.is_ascii_alphanumeric())
                .collect();
            let lowered = name.to_ascii_lowercase();
            let closing = text[at..].starts_with("</");
            if !closing && XMLDOC_BLOCK.contains(&lowered.as_str()) {
                out.push_str(
                    "

",
                );
            } else if matches!(lowered.as_str(), "p" | "br" | "li" | "tr") {
                out.push(' ');
            }
            at += width;
            continue;
        }
        let glyph = text[at..].chars().next().unwrap_or('<');
        out.push(glyph);
        at += glyph.len_utf8();
    }
    let collapsed = out
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect::<Vec<_>>()
        .join(
            "
",
        );
    collapsed
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
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
    let start = unwrap_summary(&strip_html(&unwrap_inline_tags(text)));
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
    fn an_inline_tag_becomes_the_thing_it_names() {
        assert_eq!(
            break_before_first_block_tag("Returns {@code null} when {@link Gson#fromJson} fails."),
            "Returns null when Gson#fromJson fails."
        );
    }

    #[test]
    fn javadoc_html_is_markup_rather_than_text() {
        assert_eq!(
            break_before_first_block_tag("Selects fields.<p>Types excluded are adapted.</p>"),
            "Selects fields. Types excluded are adapted."
        );
    }

    #[test]
    fn a_generic_parameter_is_not_an_html_tag() {
        let given = "Accepts a List<String> and returns Map<String, Integer>.";
        assert_eq!(break_before_first_block_tag(given), given);
    }

    #[test]
    fn an_escaped_angle_bracket_comes_back() {
        assert_eq!(
            break_before_first_block_tag("Compares a &lt; b using &amp; semantics."),
            "Compares a < b using & semantics."
        );
    }

    #[test]
    fn a_csharp_summary_is_the_claim_and_its_siblings_are_the_detail() {
        let given = "<summary>Validates a token.</summary><param name=\"raw\">the bytes</param>";
        assert_eq!(
            break_before_first_block_tag(given),
            "Validates a token.

the bytes"
        );
    }

    #[test]
    fn a_csharp_inline_reference_keeps_its_text() {
        assert_eq!(
            break_before_first_block_tag("<summary>Returns <c>null</c> when empty.</summary>"),
            "Returns null when empty."
        );
    }

    #[test]
    fn prose_without_tags_is_untouched() {
        let given = "Validation must precede tenant resolution.";
        assert_eq!(break_before_first_block_tag(given), given);
    }
}
