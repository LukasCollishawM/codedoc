use codedoc_ledger::{Assurance, Author};

use crate::OpsError;

pub fn parse_assurance(raw: Option<&str>) -> Result<Option<Assurance>, OpsError> {
    let Some(text) = raw else {
        return Ok(None);
    };
    if text.trim().is_empty() {
        return Ok(None);
    }
    Assurance::parse(text).map(Some).ok_or_else(|| OpsError::UnknownAssurance {
        found: text.to_owned(),
        vocabulary: "asserted, inferred, speculative".to_owned(),
    })
}

#[derive(Debug, Clone)]
pub struct Attribution {
    pub author: Author,
    pub assurance: Assurance,
}

impl Attribution {
    pub fn human(identity: &str) -> Self {
        Attribution {
            author: Author::Human { identity: identity.to_owned() },
            assurance: Assurance::Asserted,
        }
    }

    pub fn agent(model: &str, session: &str) -> Self {
        Attribution {
            author: Author::Agent { model: model.to_owned(), session: session.to_owned() },
            assurance: Assurance::Inferred,
        }
    }

    pub fn analyzer(name: &str) -> Self {
        Attribution {
            author: Author::Analyzer { name: name.to_owned() },
            assurance: Assurance::Inferred,
        }
    }

    pub fn runtime(name: &str) -> Self {
        Attribution {
            author: Author::Runtime { name: name.to_owned() },
            assurance: Assurance::Asserted,
        }
    }

    pub fn with_assurance(mut self, assurance: Option<Assurance>) -> Self {
        if let Some(stated) = assurance {
            self.assurance = stated;
        }
        self
    }

    pub fn is_agent(&self) -> bool {
        matches!(self.author, Author::Agent { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agents_default_to_inferred_and_humans_to_asserted() {
        assert_eq!(Attribution::agent("m", "s").assurance, Assurance::Inferred);
        assert_eq!(Attribution::human("someone").assurance, Assurance::Asserted);
    }

    #[test]
    fn a_runtime_observation_is_asserted_and_an_analyzers_inference_is_not() {
        assert_eq!(Attribution::runtime("profiler").assurance, Assurance::Asserted);
        assert_eq!(Attribution::analyzer("clippy").assurance, Assurance::Inferred);
        assert!(matches!(Attribution::runtime("profiler").author, Author::Runtime { .. }));
    }

    #[test]
    fn an_unrecognised_assurance_is_refused_rather_than_ignored() {
        assert!(parse_assurance(Some("speculatve")).is_err());
        assert_eq!(parse_assurance(Some("speculative")).unwrap(), Some(Assurance::Speculative));
        assert_eq!(parse_assurance(Some("  Asserted ")).unwrap(), Some(Assurance::Asserted));
        assert_eq!(parse_assurance(None).unwrap(), None);
        assert_eq!(parse_assurance(Some("   ")).unwrap(), None);
    }

    #[test]
    fn an_explicit_assurance_overrides_the_default() {
        let stated = Attribution::agent("m", "s").with_assurance(Some(Assurance::Asserted));
        assert_eq!(stated.assurance, Assurance::Asserted);
    }

    #[test]
    fn omitting_an_assurance_keeps_the_default() {
        let implied = Attribution::agent("m", "s").with_assurance(None);
        assert_eq!(implied.assurance, Assurance::Inferred);
    }
}
