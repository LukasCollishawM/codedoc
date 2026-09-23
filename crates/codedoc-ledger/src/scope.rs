use std::fs;
use std::path::{Path, PathBuf};

use codedoc_core::Digest;
use serde::{Deserialize, Serialize};

pub const SHARED_DIRECTORY: &str = ".codedoc";
pub const LOCAL_DIRECTORY: &str = "codedoc";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Scope {
    Shared,
    Local,
    Global,
}

impl Scope {
    pub const ALL: [Scope; 3] = [Scope::Shared, Scope::Local, Scope::Global];

    pub fn as_str(self) -> &'static str {
        match self {
            Scope::Shared => "shared",
            Scope::Local => "local",
            Scope::Global => "global",
        }
    }

    pub fn parse(text: &str) -> Option<Scope> {
        match text.trim().to_ascii_lowercase().as_str() {
            "shared" | "tracked" | "team" => Some(Scope::Shared),
            "local" | "untracked" | "private" => Some(Scope::Local),
            "global" | "machine" => Some(Scope::Global),
            _ => None,
        }
    }

    pub fn leaves_repository_evidence(self) -> bool {
        matches!(self, Scope::Shared)
    }

    pub fn describe(self) -> &'static str {
        match self {
            Scope::Shared => "committed with the repository and shared with the team",
            Scope::Local => "inside .git, private to this clone, invisible to git",
            Scope::Global => "outside the repository, private to this machine",
        }
    }

    pub fn directory(self, root: &Path) -> Option<PathBuf> {
        match self {
            Scope::Shared => Some(root.join(SHARED_DIRECTORY)),
            Scope::Local => git_directory(root).map(|git| git.join(LOCAL_DIRECTORY)),
            Scope::Global => global_directory(root),
        }
    }
}

pub fn git_directory(root: &Path) -> Option<PathBuf> {
    let marker = root.join(".git");
    if marker.is_dir() {
        return Some(marker);
    }
    if marker.is_file() {
        let contents = fs::read_to_string(&marker).ok()?;
        let target = contents.trim().strip_prefix("gitdir:")?.trim();
        let resolved = PathBuf::from(target);
        let absolute = if resolved.is_absolute() { resolved } else { root.join(resolved) };
        return absolute.is_dir().then_some(absolute);
    }
    None
}

pub fn global_directory(root: &Path) -> Option<PathBuf> {
    let canonical = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let key = Digest::of_domain("codedoc.workspace.v1", canonical.to_string_lossy().as_bytes());
    let base = global_store()?;
    Some(base.join(key.to_string()))
}

fn global_store() -> Option<PathBuf> {
    if let Ok(explicit) = std::env::var("CODEDOC_HOME") {
        return Some(PathBuf::from(explicit));
    }
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        return Some(PathBuf::from(local).join("codedoc"));
    }
    if let Ok(data) = std::env::var("XDG_DATA_HOME") {
        return Some(PathBuf::from(data).join("codedoc"));
    }
    let home = std::env::var("HOME").ok()?;
    Some(PathBuf::from(home).join(".local/share/codedoc"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aliases_resolve_to_scopes() {
        assert_eq!(Scope::parse("untracked"), Some(Scope::Local));
        assert_eq!(Scope::parse("private"), Some(Scope::Local));
        assert_eq!(Scope::parse("Shared"), Some(Scope::Shared));
        assert_eq!(Scope::parse("machine"), Some(Scope::Global));
        assert_eq!(Scope::parse("nonsense"), None);
    }

    #[test]
    fn only_the_shared_scope_touches_the_working_tree() {
        assert!(Scope::Shared.leaves_repository_evidence());
        assert!(!Scope::Local.leaves_repository_evidence());
        assert!(!Scope::Global.leaves_repository_evidence());
    }

    #[test]
    fn the_local_scope_lives_inside_the_git_directory() {
        let workspace = tempfile::tempdir().unwrap();
        fs::create_dir(workspace.path().join(".git")).unwrap();
        let directory = Scope::Local.directory(workspace.path()).unwrap();
        assert!(directory.starts_with(workspace.path().join(".git")));
    }

    #[test]
    fn the_local_scope_follows_a_worktree_pointer() {
        let workspace = tempfile::tempdir().unwrap();
        let real = workspace.path().join("actual-git-dir");
        fs::create_dir(&real).unwrap();
        fs::write(workspace.path().join(".git"), format!("gitdir: {}\n", real.display())).unwrap();
        let directory = Scope::Local.directory(workspace.path()).unwrap();
        assert!(directory.starts_with(&real));
    }

    #[test]
    fn the_local_scope_is_absent_without_git() {
        let workspace = tempfile::tempdir().unwrap();
        assert!(Scope::Local.directory(workspace.path()).is_none());
    }

    #[test]
    fn global_directories_differ_per_repository() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        assert_ne!(Scope::Global.directory(first.path()), Scope::Global.directory(second.path()));
    }
}
