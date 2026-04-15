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
            "CREATE VIRTUAL TABLE IF NOT EXISTS entries_macos
             USING fts5(description, command, tokenize='porter ascii');
             CREATE VIRTUAL TABLE IF NOT EXISTS entries_common
             USING fts5(description, command, tokenize='porter ascii');
             CREATE VIRTUAL TABLE IF NOT EXISTS user_examples
             USING fts5(description, command, tokenize='porter ascii');",
        )?;

        // One-time best-effort migration from legacy `entries` table.
        if self.table_exists("entries")? {
            let common_count: i64 = self
                .conn
                .query_row("SELECT count(*) FROM entries_common", [], |row| row.get(0))?;
            if common_count == 0 {
                self.conn.execute_batch(
                    "INSERT INTO entries_common(description, command)
                     SELECT description, command FROM entries;",
                )?;
            }
        }

        Ok(())
    }

    pub fn insert(&self, entry: &BankEntry) -> SqlResult<()> {
        self.insert_common(entry)
    }

    pub fn insert_macos(&self, entry: &BankEntry) -> SqlResult<()> {
        self.conn.execute(
            "INSERT INTO entries_macos(description, command) VALUES (?1, ?2)",
            params![entry.description, entry.command],
        )?;
        Ok(())
    }

    pub fn insert_common(&self, entry: &BankEntry) -> SqlResult<()> {
        self.conn.execute(
            "INSERT INTO entries_common(description, command) VALUES (?1, ?2)",
            params![entry.description, entry.command],
        )?;
        Ok(())
    }

    /// Delete all TLDR entries (macOS and common). Does not touch user_examples.
    pub fn clear_entries(&self) -> SqlResult<()> {
        self.conn
            .execute_batch("DELETE FROM entries_macos; DELETE FROM entries_common;")
    }

    pub fn insert_batch(&self, entries: &[BankEntry]) -> SqlResult<()> {
        for entry in entries {
            self.insert(entry)?;
        }
        Ok(())
    }

    pub fn insert_batch_macos(&self, entries: &[BankEntry]) -> SqlResult<()> {
        for entry in entries {
            self.insert_macos(entry)?;
        }
        Ok(())
    }

    pub fn insert_batch_common(&self, entries: &[BankEntry]) -> SqlResult<()> {
        for entry in entries {
            self.insert_common(entry)?;
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
        // User overrides come first, then macOS entries, then common entries.
        let mut results: Vec<BankEntry> = Vec::new();

        self.search_table_into("user_examples", query, limit, &mut results)?;
        self.search_table_into("entries_macos", query, limit, &mut results)?;
        self.search_table_into("entries_common", query, limit, &mut results)?;

        Ok(results)
    }

    pub fn len(&self) -> SqlResult<usize> {
        let macos: i64 = self
            .conn
            .query_row("SELECT count(*) FROM entries_macos", [], |row| row.get(0))?;
        let common: i64 = self
            .conn
            .query_row("SELECT count(*) FROM entries_common", [], |row| row.get(0))?;
        let count = macos + common;
        Ok(count as usize)
    }

    fn table_exists(&self, name: &str) -> SqlResult<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type IN ('table', 'view') AND name = ?1",
            params![name],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    fn search_table_into(
        &self,
        table: &str,
        query: &str,
        limit: usize,
        out: &mut Vec<BankEntry>,
    ) -> SqlResult<()> {
        if out.len() >= limit {
            return Ok(());
        }

        let remaining = (limit - out.len()) as i64;
        let sql = format!(
            "SELECT description, command FROM {table} WHERE {table} MATCH ?1 ORDER BY rank LIMIT ?2"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params![query, remaining], |row| {
            Ok(BankEntry {
                description: row.get(0)?,
                command: row.get(1)?,
            })
        })?;

        for row in rows {
            out.push(row?);
        }

        Ok(())
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

    #[test]
    fn search_prefers_macos_entries_before_common_entries() {
        let bank = Bank::open_in_memory().unwrap();

        bank.insert_common(&BankEntry {
            description: "show network interfaces".to_string(),
            command: "ip link".to_string(),
        })
        .unwrap();
        bank.insert_macos(&BankEntry {
            description: "show network interfaces".to_string(),
            command: "ifconfig".to_string(),
        })
        .unwrap();

        let results = bank.search("network interfaces", 5).unwrap();
        assert!(results.len() >= 2, "expected macOS and common matches");
        assert_eq!(results[0].command, "ifconfig");
        assert_eq!(results[1].command, "ip link");
    }

    #[test]
    fn search_uses_three_tier_order_user_then_macos_then_common() {
        let bank = Bank::open_in_memory().unwrap();

        bank.insert_common(&BankEntry {
            description: "date 90 days ago".to_string(),
            command: "date -d '-90 days' '+%Y-%m-%d'".to_string(),
        })
        .unwrap();
        bank.insert_macos(&BankEntry {
            description: "date 90 days ago".to_string(),
            command: "date -v -90d '+%Y-%m-%d'".to_string(),
        })
        .unwrap();
        bank.teach("date 90 days ago", "date -v -90d '+%Y-%m-%d'")
            .unwrap();

        let results = bank.search("date 90 days ago", 5).unwrap();
        assert!(results.len() >= 3, "expected user, macOS, and common matches");
        assert_eq!(results[0].command, "date -v -90d '+%Y-%m-%d'");
        assert_eq!(results[1].command, "date -v -90d '+%Y-%m-%d'");
        assert_eq!(results[2].command, "date -d '-90 days' '+%Y-%m-%d'");
    }
}
