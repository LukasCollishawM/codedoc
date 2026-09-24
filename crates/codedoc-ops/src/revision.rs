use std::fs;
use std::path::Path;

use codedoc_core::GitRev;

pub fn head_revision(root: &Path) -> Option<GitRev> {
    let git = root.join(".git");
    let head = fs::read_to_string(git.join("HEAD")).ok()?;
    let trimmed = head.trim();
    if let Some(reference) = trimmed.strip_prefix("ref: ") {
        let direct = git.join(reference);
        if let Ok(contents) = fs::read_to_string(direct) {
            return GitRev::parse(contents.trim()).ok();
        }
        return packed_revision(&git, reference);
    }
    GitRev::parse(trimmed).ok()
}

fn packed_revision(git: &Path, reference: &str) -> Option<GitRev> {
    let packed = fs::read_to_string(git.join("packed-refs")).ok()?;
    packed.lines().find_map(|line| {
        let (revision, name) = line.split_once(' ')?;
        (name.trim() == reference).then(|| GitRev::parse(revision).ok())?
    })
}
