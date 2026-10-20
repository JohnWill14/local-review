use local_review::notes::NoteStore;
use rusqlite::Connection;
use tempfile::tempdir;

#[test]
fn existing_single_line_notes_are_migrated_to_ranges() {
    let directory = tempdir().unwrap();
    let database = directory.path().join("notes.sqlite");
    let connection = Connection::open(&database).unwrap();
    connection.execute_batch("CREATE TABLE notes (id INTEGER PRIMARY KEY, repository TEXT NOT NULL, path TEXT NOT NULL, line INTEGER, body TEXT NOT NULL, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP); INSERT INTO notes (repository, path, line, body) VALUES ('/repo', 'src/a.rs', 9, 'legacy');").unwrap();
    drop(connection);

    let store = NoteStore::open(&database).unwrap();
    let notes = store.list("/repo").unwrap();
    assert_eq!(notes[0].start_line, Some(9));
    assert_eq!(notes[0].end_line, Some(9));
    assert_eq!(notes[0].snippet, None);
}
