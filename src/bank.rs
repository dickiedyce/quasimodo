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

    pub fn insert_batch(&self, entries: &[BankEntry]) -> SqlResult<()> {
        for entry in entries {
            self.insert(entry)?;
        }
        Ok(())
    }

    pub fn search(&self, query: &str, limit: usize) -> SqlResult<Vec<BankEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT description, command FROM entries
             WHERE entries MATCH ?1
             ORDER BY rank
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![query, limit as i64], |row| {
            Ok(BankEntry {
                description: row.get(0)?,
                command: row.get(1)?,
            })
        })?;
        rows.collect()
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
