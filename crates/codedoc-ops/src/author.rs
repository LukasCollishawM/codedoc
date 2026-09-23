use codedoc_ledger::{Assurance, Author};

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
