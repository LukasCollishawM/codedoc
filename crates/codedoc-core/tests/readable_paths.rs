use std::path::Path;

use codedoc_core::readable_path;

const VERBATIM: &str = r"\\?\C:\repo\.codedoc\ledger\76.jsonl";
const PLAIN: &str = r"C:\repo\.codedoc\ledger\76.jsonl";

#[test]
fn a_windows_extended_length_prefix_is_not_shown_to_the_reader() {
    assert_eq!(
        readable_path(Path::new(VERBATIM)),
        PLAIN,
        "fs::canonicalize returns an extended-length path on Windows, and that prefix \
         reached the reader: a conflicted ledger named its shard as a path opening with \
         a drive letter buried behind two slashes, a question mark and a third slash. \
         The prefix is an argument to the Windows API rather than part of where the \
         file is."
    );
}

#[test]
fn a_path_without_the_prefix_is_left_exactly_as_it_is() {
    assert_eq!(readable_path(Path::new(PLAIN)), PLAIN);
    assert_eq!(readable_path(Path::new("/home/dev/repo/.codedoc")), "/home/dev/repo/.codedoc");
    assert_eq!(readable_path(Path::new("relative/path")), "relative/path");
}

#[test]
fn a_network_share_keeps_both_of_its_leading_slashes() {
    assert_eq!(
        readable_path(Path::new(r"\\?\UNC\server\share\repo")),
        r"\\server\share\repo",
        "the verbatim form spells a share with UNC where the leading slashes belong, so \
         removing only the prefix would leave a path naming a directory called server"
    );
}
