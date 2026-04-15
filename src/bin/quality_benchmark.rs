use std::collections::BTreeMap;

use quasimodo::{CliArgs, OllamaAdapter, ProviderAdapter, run};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
enum Category {
    Files,
    Network,
    Git,
    MacOs,
    Shell,
}

impl Category {
    fn as_str(self) -> &'static str {
        match self {
            Category::Files => "files",
            Category::Network => "network",
            Category::Git => "git",
            Category::MacOs => "macos",
            Category::Shell => "shell",
        }
    }
}

#[derive(Clone, Copy)]
struct Case {
    category: Category,
    prompt: &'static str,
    // First token prefixes that count as acceptable answers for this prompt.
    expected_prefixes: &'static [&'static str],
}

const CASES: &[Case] = &[
    Case {
        category: Category::Files,
        prompt: "show disk usage",
        expected_prefixes: &["du", "df"],
    },
    Case {
        category: Category::Files,
        prompt: "find files changed in the last hour",
        expected_prefixes: &["find", "git"],
    },
    Case {
        category: Category::Network,
        prompt: "list open network ports",
        expected_prefixes: &["lsof", "netstat"],
    },
    Case {
        category: Category::MacOs,
        prompt: "copy hello to clipboard",
        expected_prefixes: &["echo", "pbcopy"],
    },
    Case {
        category: Category::MacOs,
        prompt: "prevent mac from sleeping",
        expected_prefixes: &["caffeinate", "pmset"],
    },
    Case {
        category: Category::Files,
        prompt: "show largest files in current folder",
        expected_prefixes: &["find", "du", "ls"],
    },
    Case {
        category: Category::Files,
        prompt: "count lines in all rust files",
        expected_prefixes: &["find", "rg", "wc"],
    },
    Case {
        category: Category::Files,
        prompt: "check disk free space",
        expected_prefixes: &["df"],
    },
    Case {
        category: Category::Files,
        prompt: "list files recursively",
        expected_prefixes: &["find", "ls", "tree"],
    },
    Case {
        category: Category::Files,
        prompt: "find empty files",
        expected_prefixes: &["find"],
    },
    Case {
        category: Category::Files,
        prompt: "show top 10 largest files",
        expected_prefixes: &["find", "du", "ls"],
    },
    Case {
        category: Category::Files,
        prompt: "show folder sizes one level deep",
        expected_prefixes: &["du"],
    },
    Case {
        category: Category::Network,
        prompt: "show listening tcp ports",
        expected_prefixes: &["lsof", "netstat"],
    },
    Case {
        category: Category::Network,
        prompt: "resolve github.com dns",
        expected_prefixes: &["dig", "nslookup", "host"],
    },
    Case {
        category: Category::Network,
        prompt: "test if port 443 is open on example.com",
        expected_prefixes: &["nc", "nmap", "telnet"],
    },
    Case {
        category: Category::Network,
        prompt: "show current network interfaces",
        expected_prefixes: &["ifconfig", "networksetup"],
    },
    Case {
        category: Category::Git,
        prompt: "show last 5 commits",
        expected_prefixes: &["git"],
    },
    Case {
        category: Category::Git,
        prompt: "create and switch to a new branch named feature-x",
        expected_prefixes: &["git"],
    },
    Case {
        category: Category::Git,
        prompt: "show files changed in current branch",
        expected_prefixes: &["git"],
    },
    Case {
        category: Category::Git,
        prompt: "discard unstaged changes in one file",
        expected_prefixes: &["git"],
    },
    Case {
        category: Category::MacOs,
        prompt: "open current folder in Finder",
        expected_prefixes: &["open"],
    },
    Case {
        category: Category::MacOs,
        prompt: "resize png image to 800x600 on mac",
        expected_prefixes: &["sips", "convert", "magick"],
    },
    Case {
        category: Category::MacOs,
        prompt: "resize svg image to 800x600 on mac",
        expected_prefixes: &["convert", "magick", "rsvg-convert"],
    },
    Case {
        category: Category::MacOs,
        prompt: "show battery status",
        expected_prefixes: &["pmset"],
    },
    Case {
        category: Category::MacOs,
        prompt: "copy current directory path to clipboard",
        expected_prefixes: &["pwd", "echo", "pbcopy"],
    },
    Case {
        category: Category::Shell,
        prompt: "extract file.tar.gz",
        expected_prefixes: &["tar"],
    },
    Case {
        category: Category::Shell,
        prompt: "search for TODO in rust files",
        expected_prefixes: &["rg", "grep"],
    },
    Case {
        category: Category::Shell,
        prompt: "print current shell",
        expected_prefixes: &["echo", "ps"],
    },
    Case {
        category: Category::Shell,
        prompt: "show process using most cpu",
        expected_prefixes: &["ps", "top"],
    },
    Case {
        category: Category::Shell,
        prompt: "print only unique sorted lines from file.txt",
        expected_prefixes: &["sort", "uniq", "awk"],
    },
];

#[derive(Default, Clone, Copy)]
struct CategoryStats {
    total: usize,
    pass_no_retry: usize,
    pass_with_retry: usize,
}

fn first_token(s: &str) -> &str {
    s.split_whitespace().next().unwrap_or("")
}

fn score_case(output: &str, case: Case) -> bool {
    let token = first_token(output).to_lowercase();
    case.expected_prefixes.iter().any(|p| token.starts_with(p))
}

fn run_one(adapter: &OllamaAdapter, prompt: &str, bank_path: Option<String>, retry: bool) -> String {
    let args = CliArgs {
        model: "llama3.2".to_string(),
        endpoint: "http://localhost:11434".to_string(),
        prompt: prompt.to_string(),
        bank_path,
        notfound: false,
        explain: false,
        samples: 1,
        temperature: 0.0,
        stdin: false,
        system_prompt: None,
        history_file: None,
        quality_retry: retry,
        teach_description: None,
        teach_command: None,
        delete_taught: None,
        help: false,
        describe: false,
        list_taught: false,
    };

    match run(&args, adapter) {
        Ok(out) => out,
        Err(err) => format!("ERROR: {err:?}"),
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let bank_path = args.next().or_else(|| {
        let candidate = "tldr_bank.db".to_string();
        if std::path::Path::new(&candidate).exists() {
            Some(candidate)
        } else {
            None
        }
    });

    let adapter = match OllamaAdapter::new("http://localhost:11434") {
        Ok(a) => a,
        Err(err) => {
            eprintln!("failed to create adapter: {err:?}");
            std::process::exit(1);
        }
    };

    match adapter.is_local_available() {
        Ok(true) => {}
        Ok(false) => {
            eprintln!("ollama is not available at http://localhost:11434");
            std::process::exit(1);
        }
        Err(err) => {
            eprintln!("unable to check ollama availability: {err:?}");
            std::process::exit(1);
        }
    }

    println!("Prompt benchmark ({} cases)", CASES.len());
    if let Some(path) = &bank_path {
        println!("Using bank: {path}");
    } else {
        println!("Using bank: <none>");
    }
    println!();

    let mut pass_no_retry = 0usize;
    let mut pass_with_retry = 0usize;
    let mut by_category: BTreeMap<Category, CategoryStats> = BTreeMap::new();

    for (i, case) in CASES.iter().copied().enumerate() {
        let out_no_retry = run_one(&adapter, case.prompt, bank_path.clone(), false);
        let out_with_retry = run_one(&adapter, case.prompt, bank_path.clone(), true);

        let ok_no_retry = score_case(&out_no_retry, case);
        let ok_with_retry = score_case(&out_with_retry, case);

        if ok_no_retry {
            pass_no_retry += 1;
        }
        if ok_with_retry {
            pass_with_retry += 1;
        }

        let entry = by_category.entry(case.category).or_default();
        entry.total += 1;
        if ok_no_retry {
            entry.pass_no_retry += 1;
        }
        if ok_with_retry {
            entry.pass_with_retry += 1;
        }

        println!("{}. [{}] {}", i + 1, case.category.as_str(), case.prompt);
        println!("   no-retry : {}{}", out_no_retry, if ok_no_retry { "  [ok]" } else { "  [miss]" });
        println!("   retry    : {}{}", out_with_retry, if ok_with_retry { "  [ok]" } else { "  [miss]" });
    }

    let total = CASES.len() as f32;
    let pct_no = (pass_no_retry as f32 / total) * 100.0;
    let pct_yes = (pass_with_retry as f32 / total) * 100.0;

    println!();
    println!("Summary:");
    println!("- no-retry pass rate : {}/{} ({:.1}%)", pass_no_retry, CASES.len(), pct_no);
    println!("- retry pass rate    : {}/{} ({:.1}%)", pass_with_retry, CASES.len(), pct_yes);
    println!("- delta              : {:+.1} pp", pct_yes - pct_no);

    println!();
    println!("By category:");
    for (category, stats) in by_category {
        let total = stats.total as f32;
        let no = (stats.pass_no_retry as f32 / total) * 100.0;
        let yes = (stats.pass_with_retry as f32 / total) * 100.0;
        println!(
            "- {:<8} no-retry: {}/{} ({:.1}%), retry: {}/{} ({:.1}%), delta: {:+.1} pp",
            category.as_str(),
            stats.pass_no_retry,
            stats.total,
            no,
            stats.pass_with_retry,
            stats.total,
            yes,
            yes - no
        );
    }
}
