use rusqlite::{Connection, Result as SqlResult, params};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BankEntry {
    pub description: String,
    pub command: String,
}

pub struct Bank {
    conn: Connection,
}

impl Bank {
    pub fn open_in_memory() -> SqlResult<Self> {
        let conn = Connection::open_in_memory()?;
        let bank = Self { conn };
        bank.init_schema()?;
        Ok(bank)
    }

    pub fn open(path: &str) -> SqlResult<Self> {
        let conn = Connection::open(path)?;
        let bank = Self { conn };
        bank.init_schema()?;
        Ok(bank)
    }

    fn init_schema(&self) -> SqlResult<()> {
        self.conn.execute_batch(
            "CREATE VIRTUAL TABLE IF NOT EXISTS entries
             USING fts5(description, command, tokenize='porter ascii');
             CREATE VIRTUAL TABLE IF NOT EXISTS user_examples
             USING fts5(description, command, tokenize='porter ascii');",
        )
    }

    pub fn insert(&self, entry: &BankEntry) -> SqlResult<()> {
        self.conn.execute(
            "INSERT INTO entries(description, command) VALUES (?1, ?2)",
            params![entry.description, entry.command],
        )?;
        Ok(())
    }

    /// Delete all TLDR entries. Does not touch user_examples.
    pub fn clear_entries(&self) -> SqlResult<()> {
        self.conn.execute_batch("DELETE FROM entries;")
    }

    pub fn insert_batch(&self, entries: &[BankEntry]) -> SqlResult<()> {
        for entry in entries {
            self.insert(entry)?;
        }
        Ok(())
    }

    /// Teach the bank a new user-curated prompt→command example.
    /// User examples are stored separately from TLDR data and are never
    /// overwritten by `build-bank`. They are prioritised over TLDR entries
    /// in retrieval results.
    pub fn teach(&self, description: &str, command: &str) -> SqlResult<()> {
        self.conn.execute(
            "INSERT INTO user_examples(description, command) VALUES (?1, ?2)",
            params![description, command],
        )?;
        Ok(())
    }

    pub fn search(&self, query: &str, limit: usize) -> SqlResult<Vec<BankEntry>> {
        // User examples come first, then TLDR entries, total capped at limit.
        let mut results: Vec<BankEntry> = Vec::new();

        let mut stmt = self.conn.prepare(
            "SELECT description, command FROM user_examples
             WHERE user_examples MATCH ?1
             ORDER BY rank
             LIMIT ?2",
        )?;
        let user_rows = stmt.query_map(params![query, limit as i64], |row| {
            Ok(BankEntry {
                description: row.get(0)?,
                command: row.get(1)?,
            })
        })?;
        for row in user_rows {
            results.push(row?);
        }

        if results.len() < limit {
            let remaining = (limit - results.len()) as i64;
            let mut stmt2 = self.conn.prepare(
                "SELECT description, command FROM entries
                 WHERE entries MATCH ?1
                 ORDER BY rank
                 LIMIT ?2",
            )?;
            let tldr_rows = stmt2.query_map(params![query, remaining], |row| {
                Ok(BankEntry {
                    description: row.get(0)?,
                    command: row.get(1)?,
                })
            })?;
            for row in tldr_rows {
                results.push(row?);
            }
        }

        Ok(results)
    }

    pub fn len(&self) -> SqlResult<usize> {
        let count: i64 =
            self.conn.query_row("SELECT count(*) FROM entries", [], |row| row.get(0))?;
        Ok(count as usize)
    }
}

pub struct Retriever<'a> {
    bank: &'a Bank,
    limit: usize,
}

impl<'a> Retriever<'a> {
    pub fn new(bank: &'a Bank, limit: usize) -> Self {
        Self { bank, limit }
    }

    pub fn retrieve(&self, query: &str) -> SqlResult<Vec<BankEntry>> {
        self.bank.search(query, self.limit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn teach_stores_user_example() {
        let bank = Bank::open_in_memory().unwrap();
        bank.teach("what is the date 90 days ago", "date -v -90d '+%Y-%m-%d'").unwrap();
        let results = bank.search("90 days ago", 5).unwrap();
        assert!(!results.is_empty(), "should find taught example");
        assert_eq!(results[0].command, "date -v -90d '+%Y-%m-%d'");
    }

    #[test]
    fn teach_result_appears_before_tldr_entries() {
        let bank = Bank::open_in_memory().unwrap();
        // Add a TLDR-style entry first
        bank.insert(&BankEntry {
            description: "date 90 days ago linux".into(),
            command: "date -d '-90 days' '+%Y-%m-%d'".into(),
        }).unwrap();
        // Then teach the correct macOS version
        bank.teach("what is the date 90 days ago", "date -v -90d '+%Y-%m-%d'").unwrap();
        let results = bank.search("90 days ago", 5).unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].command, "date -v -90d '+%Y-%m-%d'",
            "user example should appear first");
    }

    #[test]
    fn teach_survives_clear_entries() {
        let bank = Bank::open_in_memory().unwrap();

        bank.insert(&BankEntry {
            description: "list files".to_string(),
            command: "ls".to_string(),
        }).unwrap();
        bank.teach("show date", "date").unwrap();

        bank.clear_entries().unwrap();

        // TLDR entry is gone
        let tldr = bank.search("list files", 5).unwrap();
        assert!(tldr.is_empty(), "entries should be cleared");

        // User example survives
        let user = bank.search("show date", 5).unwrap();
        assert!(!user.is_empty(), "user examples should survive clear_entries");
        assert_eq!(user[0].command, "date");
    }
}
