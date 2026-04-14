/// Build the tldr-pages example bank.
///
/// Usage: build-bank <output.db> [<tldr-pages-dir>]
///
/// If no tldr-pages directory is given, the script clones from GitHub unless
/// a `TLDR_DIR` environment variable is set.
use quasimodo::bank::{Bank, BankEntry};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

fn main() {
    let mut args = std::env::args().skip(1);

    let db_path = args.next().unwrap_or_else(|| "tldr_bank.db".to_string());
    let tldr_dir: PathBuf = args
        .next()
        .or_else(|| std::env::var("TLDR_DIR").ok())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let path = PathBuf::from("/tmp/tldr-pages");
            if !path.exists() {
                eprintln!("Cloning tldr-pages into /tmp/tldr-pages …");
                let status = Command::new("git")
                    .args(["clone", "--depth=1", "https://github.com/tldr-pages/tldr", "/tmp/tldr-pages"])
                    .status()
                    .expect("git clone failed");
                if !status.success() {
                    eprintln!("error: git clone failed");
                    std::process::exit(1);
                }
            }
            path
        });

    eprintln!("Parsing tldr pages from {:?} …", tldr_dir);
    let entries = parse_tldr_dir(&tldr_dir);
    eprintln!("Parsed {} entries", entries.len());

    let bank = Bank::open(&db_path).expect("failed to open database");
    bank.clear_entries().expect("failed to clear existing entries");
    bank.insert_batch(&entries).expect("failed to insert entries");
    eprintln!("Written to {db_path}");
}

/// Walk a tldr-pages directory tree and parse every .md file into BankEntry pairs.
fn parse_tldr_dir(root: &Path) -> Vec<BankEntry> {
    let mut entries = Vec::new();
    for md_file in find_md_files(root) {
        let Ok(text) = fs::read_to_string(&md_file) else {
            continue;
        };
        entries.extend(parse_tldr_page(&text));
    }
    entries
}

fn find_md_files(root: &Path) -> Vec<PathBuf> {
    let mut results = Vec::new();
    if let Ok(iter) = fs::read_dir(root) {
        for entry in iter.flatten() {
            let path = entry.path();
            if path.is_dir() {
                results.extend(find_md_files(&path));
            } else if path.extension().map(|e| e == "md").unwrap_or(false) {
                results.push(path);
            }
        }
    }
    results
}

/// Parse a single tldr page markdown source into BankEntry items.
///
/// tldr format:
///   # command-name
///   > Short description.
///   > More description.
///   - Example description:
///   `actual command`
fn parse_tldr_page(text: &str) -> Vec<BankEntry> {
    let mut entries = Vec::new();
    let mut current_desc: Option<String> = None;

    for line in text.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("- ") {
            // Description line (strip trailing colon)
            current_desc = Some(trimmed[2..].trim_end_matches(':').trim().to_string());
        } else if trimmed.starts_with('`') && trimmed.ends_with('`') && trimmed.len() > 1 {
            // Command line
            if let Some(desc) = current_desc.take() {
                let command = trimmed[1..trimmed.len() - 1].to_string();
                if !desc.is_empty() && !command.is_empty() {
                    entries.push(BankEntry { description: desc, command });
                }
            }
        }
    }

    entries
}

#[cfg(test)]
mod tests {
    use super::parse_tldr_page;

    #[test]
    fn parses_description_and_command_pairs() {
        let page = r#"# find
> Search for files.
- Find files changed in last hour:
`find . -mmin -60`
- Show disk usage:
`du -sh .`
"#;
        let entries = parse_tldr_page(page);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].description, "Find files changed in last hour");
        assert_eq!(entries[0].command, "find . -mmin -60");
        assert_eq!(entries[1].command, "du -sh .");
    }

    #[test]
    fn skips_entries_with_no_preceding_description() {
        let page = "`orphan command`\n- desc:\n`proper`\n";
        let entries = parse_tldr_page(page);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].command, "proper");
    }
}
