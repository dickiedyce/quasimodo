use quasimodo::{CliArgs, OllamaAdapter, ProviderAdapter, run};

#[derive(Clone, Copy)]
struct Case {
    prompt: &'static str,
    // First token prefixes that count as acceptable answers for this prompt.
    expected_prefixes: &'static [&'static str],
}

const CASES: &[Case] = &[
    Case {
        prompt: "show disk usage",
        expected_prefixes: &["du", "df"],
    },
    Case {
        prompt: "find files changed in the last hour",
        expected_prefixes: &["find", "git"],
    },
    Case {
        prompt: "list open network ports",
        expected_prefixes: &["lsof", "netstat"],
    },
    Case {
        prompt: "copy hello to clipboard",
        expected_prefixes: &["echo", "pbcopy"],
    },
    Case {
        prompt: "prevent mac from sleeping",
        expected_prefixes: &["caffeinate", "pmset"],
    },
    Case {
        prompt: "show largest files in current folder",
        expected_prefixes: &["find", "du", "ls"],
    },
    Case {
        prompt: "count lines in all rust files",
        expected_prefixes: &["find", "rg", "wc"],
    },
    Case {
        prompt: "check disk free space",
        expected_prefixes: &["df"],
    },
];

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

        println!("{}. {}", i + 1, case.prompt);
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
}
