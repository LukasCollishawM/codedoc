use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::{ParseError, PathError};

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct GitRev(String);

impl GitRev {
    pub fn parse(text: &str) -> Result<Self, ParseError> {
        let acceptable_length = (7..=64).contains(&text.len());
        let hexadecimal = text.chars().all(|character| character.is_ascii_hexdigit());
        if !acceptable_length || !hexadecimal {
            return Err(ParseError::GitRevision { found: text.to_owned() });
        }
        Ok(GitRev(text.to_ascii_lowercase()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for GitRev {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl fmt::Debug for GitRev {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "GitRev({})", self.0)
    }
}

impl FromStr for GitRev {
    type Err = ParseError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        GitRev::parse(text)
    }
}

impl TryFrom<String> for GitRev {
    type Error = ParseError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        GitRev::parse(&text)
    }
}

impl From<GitRev> for String {
    fn from(revision: GitRev) -> String {
        revision.0
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SymbolPath {
    language: String,
    segments: Vec<String>,
}

impl SymbolPath {
    pub fn parse(text: &str) -> Result<Self, ParseError> {
        let Some((language, trail)) = text.split_once("://") else {
            return Err(ParseError::SymbolPathShape { found: text.to_owned() });
        };
        if language.is_empty()
            || !language
                .chars()
                .all(|character| character.is_ascii_lowercase() || character.is_ascii_digit())
        {
            return Err(ParseError::SymbolPathLanguage { found: language.to_owned() });
        }
        if trail.is_empty() {
            return Err(ParseError::SymbolPathShape { found: text.to_owned() });
        }
        let mut segments = Vec::new();
        for segment in trail.split('/') {
            if segment.is_empty() {
                return Err(ParseError::SymbolPathSegment { found: text.to_owned() });
            }
            segments.push(segment.to_owned());
        }
        Ok(SymbolPath { language: language.to_owned(), segments })
    }

    pub fn language(&self) -> &str {
        &self.language
    }

    pub fn segments(&self) -> &[String] {
        &self.segments
    }

    pub fn parent(&self) -> Option<SymbolPath> {
        if self.segments.len() <= 1 {
            return None;
        }
        Some(SymbolPath {
            language: self.language.clone(),
            segments: self.segments[..self.segments.len() - 1].to_vec(),
        })
    }

    pub fn leaf(&self) -> &str {
        self.segments.last().map(String::as_str).unwrap_or_default()
    }
}

impl fmt::Display for SymbolPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}://{}", self.language, self.segments.join("/"))
    }
}

impl fmt::Debug for SymbolPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "SymbolPath({self})")
    }
}

impl FromStr for SymbolPath {
    type Err = ParseError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        SymbolPath::parse(text)
    }
}

impl TryFrom<String> for SymbolPath {
    type Error = ParseError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        SymbolPath::parse(&text)
    }
}

impl From<SymbolPath> for String {
    fn from(path: SymbolPath) -> String {
        path.to_string()
    }
}

const RESERVED_DEVICE_NAMES: [&str; 22] = [
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RepoPath(String);

impl RepoPath {
    pub fn parse(text: &str) -> Result<Self, PathError> {
        let normalised = text.replace('\\', "/");
        if normalised.starts_with('/') || normalised.starts_with("//") {
            return Err(PathError::Absolute { found: text.to_owned() });
        }
        if normalised.contains(':') {
            return Err(PathError::Absolute { found: text.to_owned() });
        }
        let mut components = Vec::new();
        for component in normalised.split('/') {
            match component {
                "" | "." => continue,
                ".." => return Err(PathError::EscapesRoot { found: text.to_owned() }),
                named => {
                    let stem = named.split('.').next().unwrap_or(named).to_ascii_lowercase();
                    if RESERVED_DEVICE_NAMES.contains(&stem.as_str()) {
                        return Err(PathError::EscapesRoot { found: text.to_owned() });
                    }
                    if named.ends_with(' ') || named.ends_with('.') {
                        return Err(PathError::EscapesRoot { found: text.to_owned() });
                    }
                    components.push(named);
                }
            }
        }
        if components.is_empty() {
            return Err(PathError::EscapesRoot { found: text.to_owned() });
        }
        Ok(RepoPath(components.join("/")))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn extension(&self) -> Option<&str> {
        self.0.rsplit_once('.').map(|(_, extension)| extension)
    }
}

impl fmt::Display for RepoPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl fmt::Debug for RepoPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "RepoPath({})", self.0)
    }
}

impl FromStr for RepoPath {
    type Err = PathError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        RepoPath::parse(text)
    }
}

impl TryFrom<String> for RepoPath {
    type Error = PathError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        RepoPath::parse(&text)
    }
}

impl From<RepoPath> for String {
    fn from(path: RepoPath) -> String {
        path.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_revision_accepts_short_and_full_forms() {
        assert!(GitRev::parse("4fb8721").is_ok());
        assert!(GitRev::parse(&"a".repeat(64)).is_ok());
    }

    #[test]
    fn git_revision_rejects_non_hexadecimal() {
        assert!(GitRev::parse("zzzzzzz").is_err());
        assert!(GitRev::parse("abc").is_err());
    }

    #[test]
    fn symbol_path_round_trips() {
        let path = SymbolPath::parse("csharp://PaymentService/AuthorizeAsync").unwrap();
        assert_eq!(path.language(), "csharp");
        assert_eq!(path.leaf(), "AuthorizeAsync");
        assert_eq!(path.to_string(), "csharp://PaymentService/AuthorizeAsync");
    }

    #[test]
    fn symbol_path_parent_walks_upward() {
        let path = SymbolPath::parse("rust://module/Type/method").unwrap();
        let parent = path.parent().unwrap();
        assert_eq!(parent.to_string(), "rust://module/Type");
        assert!(SymbolPath::parse("rust://only").unwrap().parent().is_none());
    }

    #[test]
    fn symbol_path_rejects_malformed_input() {
        assert!(SymbolPath::parse("no-scheme").is_err());
        assert!(SymbolPath::parse("Rust://Upper").is_err());
        assert!(SymbolPath::parse("rust://").is_err());
        assert!(SymbolPath::parse("rust://a//b").is_err());
    }

    #[test]
    fn repo_path_rejects_traversal() {
        assert!(RepoPath::parse("../secrets").is_err());
        assert!(RepoPath::parse("src/../../etc/passwd").is_err());
    }

    #[test]
    fn repo_path_rejects_absolute_and_windows_roots() {
        assert!(RepoPath::parse("/etc/passwd").is_err());
        assert!(RepoPath::parse("C:/Windows").is_err());
        assert!(RepoPath::parse("//server/share").is_err());
    }

    #[test]
    fn repo_path_rejects_windows_device_names_and_streams() {
        assert!(RepoPath::parse("src/NUL").is_err());
        assert!(RepoPath::parse("src/con.txt").is_err());
        assert!(RepoPath::parse("src/file.txt:stream").is_err());
        assert!(RepoPath::parse("src/trailing.").is_err());
    }

    #[test]
    fn repo_path_normalises_separators() {
        let path = RepoPath::parse("src\\codedoc\\lib.rs").unwrap();
        assert_eq!(path.as_str(), "src/codedoc/lib.rs");
        assert_eq!(path.extension(), Some("rs"));
    }
}
