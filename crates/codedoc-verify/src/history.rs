use std::path::Path;
use std::process::Command;

use codedoc_core::GitRev;

pub fn renamed_to(root: &Path, from: &GitRev, path: &str) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["diff", "--name-status", "--find-renames", "--diff-filter=R", from.as_str(), "HEAD"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let listing = String::from_utf8_lossy(&output.stdout);
    listing.lines().find_map(|line| {
        let mut fields = line.split('\t');
        let status = fields.next()?;
        if !status.starts_with('R') {
            return None;
        }
        let previous = fields.next()?;
        let current = fields.next()?;
        (previous == path).then(|| current.to_owned())
    })
}

pub fn changed_since(root: &Path, revision: &str) -> Option<Vec<String>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["diff", "--name-only", revision, "--"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let listing = String::from_utf8_lossy(&output.stdout);
    let mut files: Vec<String> = listing.lines().map(str::to_owned).collect();

    if let Ok(untracked) = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["ls-files", "--others", "--exclude-standard"])
        .output()
        && untracked.status.success()
    {
        files.extend(String::from_utf8_lossy(&untracked.stdout).lines().map(str::to_owned));
    }
    files.sort();
    files.dedup();
    Some(files)
}

pub fn is_available(root: &Path) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "--git-dir"])
        .output()
        .is_ok_and(|output| output.status.success())
}
