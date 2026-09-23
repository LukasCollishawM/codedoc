#![no_main]

use codedoc_anchor::{Anchor, FileIndex};
use codedoc_core::RepoPath;
use codedoc_lang::Registry;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(source) = std::str::from_utf8(data) else {
        return;
    };
    let Some(adapter) = Registry::by_name("rust") else {
        return;
    };
    let Ok(tree) = adapter.parse(source) else {
        return;
    };
    let Ok(path) = RepoPath::parse("src/lib.rs") else {
        return;
    };
    let Some(node) = tree.root_node().child(0) else {
        return;
    };

    let anchor = Anchor::capture(path, adapter, source, node);
    let index = FileIndex::build(adapter, source, &tree);
    let _ = index.resolve(&anchor);
});
