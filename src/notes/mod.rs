use anyhow::Result;
use rusqlite::{params, Connection};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Note {
    pub id: i64,
    pub path: String,
    pub start_line: Option<u32>,
    pub end_line: Option<u32>,
    pub snippet: Option<String>,
    pub body: String,
}

pub struct NoteStore {
    connection: Connection,
}

impl NoteStore {
    pub fn open(path: &Path) -> Result<Self> {
        let connection = Connection::open(path)?;
        connection.execute_batch("CREATE TABLE IF NOT EXISTS notes (id INTEGER PRIMARY KEY, repository TEXT NOT NULL, path TEXT NOT NULL, line INTEGER, start_line INTEGER, end_line INTEGER, snippet TEXT, body TEXT NOT NULL, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);")?;
        migrate_range_columns(&connection)?;
        Ok(Self { connection })
    }
    pub fn list(&self, repository: &str) -> Result<Vec<Note>> {
        let mut statement = self.connection.prepare(
            "SELECT id, path, start_line, end_line, snippet, body FROM notes WHERE repository = ?1 ORDER BY id DESC",
        )?;
        let rows = statement.query_map([repository], |row| {
            Ok(Note {
                id: row.get(0)?,
                path: row.get(1)?,
                start_line: row.get(2)?,
                end_line: row.get(3)?,
                snippet: row.get(4)?,
                body: row.get(5)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }
    pub fn add(
        &self,
        repository: &str,
        path: &str,
        start_line: u32,
        end_line: u32,
        snippet: &str,
        body: &str,
    ) -> Result<()> {
        self.connection.execute(
            "INSERT INTO notes (repository, path, line, start_line, end_line, snippet, body) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![repository, path, start_line, start_line, end_line, snippet, body],
        )?;
        Ok(())
    }
    pub fn clear(&self, repository: &str) -> Result<usize> {
        Ok(self
            .connection
            .execute("DELETE FROM notes WHERE repository = ?1", [repository])?)
    }
    pub fn update(&self, id: i64, body: &str) -> Result<()> {
        self.connection.execute(
            "UPDATE notes SET body = ?1 WHERE id = ?2",
            params![body, id],
        )?;
        Ok(())
    }
}

pub fn format_markdown(notes: &[Note]) -> String {
    let mut output = String::new();
    for note in notes {
        let range = match (note.start_line, note.end_line) {
            (Some(start), Some(end)) if start != end => format!("{start}–{end}"),
            (Some(line), _) => line.to_string(),
            _ => "?".into(),
        };
        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str(&format!("## {}:{}\n", note.path, range));
        if let Some(snippet) = note
            .snippet
            .as_deref()
            .filter(|snippet| !snippet.is_empty())
        {
            output.push_str(&format!("\n```diff\n{snippet}\n```\n"));
        }
        output.push_str(&format!("\n{}\n", note.body));
    }
    output
}

fn migrate_range_columns(connection: &Connection) -> Result<()> {
    let mut statement = connection.prepare("PRAGMA table_info(notes)")?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if !columns.iter().any(|column| column == "start_line") {
        connection.execute_batch("ALTER TABLE notes ADD COLUMN start_line INTEGER")?;
    }
    if !columns.iter().any(|column| column == "end_line") {
        connection.execute_batch("ALTER TABLE notes ADD COLUMN end_line INTEGER")?;
    }
    if !columns.iter().any(|column| column == "snippet") {
        connection.execute_batch("ALTER TABLE notes ADD COLUMN snippet TEXT")?;
    }
    connection.execute_batch("UPDATE notes SET start_line = line WHERE start_line IS NULL; UPDATE notes SET end_line = COALESCE(start_line, line) WHERE end_line IS NULL;")?;
    Ok(())
}
