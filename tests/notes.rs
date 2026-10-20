use local_review::notes::{format_markdown, NoteStore};
use tempfile::tempdir;

#[test]
fn notes_are_scoped_to_the_repository() {
    let directory = tempdir().unwrap();
    let store = NoteStore::open(&directory.path().join("notes.sqlite")).unwrap();
    store
        .add(
            "/one",
            "src/lib.rs",
            18,
            22,
            "+limit = 20",
            "revisar limite",
        )
        .unwrap();
    store
        .add("/two", "src/lib.rs", 7, 7, " context", "outro projeto")
        .unwrap();

    let notes = store.list("/one").unwrap();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].path, "src/lib.rs");
    assert_eq!(notes[0].start_line, Some(18));
    assert_eq!(notes[0].end_line, Some(22));
    assert_eq!(notes[0].body, "revisar limite");
    assert_eq!(notes[0].snippet.as_deref(), Some("+limit = 20"));

    let markdown = format_markdown(&notes);
    assert!(markdown.contains("## src/lib.rs:18–22"));
    let snippet_index = markdown.find("```diff\n+limit = 20\n```").unwrap();
    let comment_index = markdown.find("revisar limite").unwrap();
    assert!(snippet_index < comment_index);
    assert!(!markdown.starts_with("# Contexto"));
    assert_eq!(store.clear("/one").unwrap(), 1);
    assert!(store.list("/one").unwrap().is_empty());
}
