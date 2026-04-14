/// --notfound mode: given a command that was not found, produce a suggestion.
///
/// Three-tier logic matching hunch's approach:
///   1. Typo: levenshtein distance ≤ 2 against known commands in bank
///   2. Installable: static map of common tools → brew formula
///   3. Linux→macOS: static map of Linux commands → macOS equivalents

use crate::bank::Bank;

// Static map of installable tools → Homebrew formula (subset, extend as needed)
const BREW_MAP: &[(&str, &str)] = &[
    ("ncdu", "ncdu"),
    ("htop", "htop"),
    ("jq", "jq"),
    ("fzf", "fzf"),
    ("ripgrep", "ripgrep"),
    ("rg", "ripgrep"),
    ("fd", "fd"),
    ("bat", "bat"),
    ("tldr", "tldr"),
    ("tree", "tree"),
    ("wget", "wget"),
    ("nvim", "neovim"),
    ("neovim", "neovim"),
    ("tmux", "tmux"),
    ("watch", "watch"),
    ("nmap", "nmap"),
    ("zoxide", "zoxide"),
    ("dust", "du-dust"),
    ("exa", "exa"),
    ("lsd", "lsd"),
];

// Static map of Linux commands → macOS equivalents
const LINUX_TO_MACOS: &[(&str, &str)] = &[
    ("ip", "ifconfig"),
    ("ifconfig", "ifconfig"),
    ("apt", "brew"),
    ("apt-get", "brew"),
    ("yum", "brew"),
    ("dnf", "brew"),
    ("pacman", "brew"),
    ("service", "launchctl"),
    ("systemctl", "launchctl"),
    ("updatedb", "sudo /usr/libexec/locate.updatedb"),
    ("locate", "/usr/bin/locate"),
    ("free", "vm_stat"),
    ("lsb_release", "sw_vers"),
    ("nproc", "sysctl -n hw.logicalcpu"),
    ("xdg-open", "open"),
    ("pbcopy", "pbcopy"),
    ("pbpaste", "pbpaste"),
    ("xclip", "pbcopy"),
    ("xsel", "pbpaste"),
    ("md5sum", "md5"),
    ("sha256sum", "shasum -a 256"),
    ("realpath", "realpath"),
    ("timeout", "gtimeout"),
    ("readlink", "greadlink"),
    ("grep", "grep"),
    ("sed", "gsed"),
    ("awk", "gawk"),
];

// Common command typo candidates to check against
const KNOWN_COMMANDS: &[&str] = &[
    "git", "grep", "find", "ls", "cd", "cp", "mv", "rm", "cat", "echo",
    "curl", "wget", "ssh", "scp", "tar", "zip", "unzip", "make", "cargo",
    "python", "python3", "pip", "pip3", "node", "npm", "npx",
    "brew", "open", "pbcopy", "pbpaste", "caffeinate", "pmset", "sips",
    "osascript", "launchctl", "defaults", "say", "afplay",
    "docker", "kubectl", "helm", "terraform", "ansible",
    "vim", "nvim", "nano", "emacs", "code", "subl",
    "rsync", "mount", "umount", "df", "du", "ps", "top", "kill",
    "awk", "sed", "sort", "uniq", "wc", "head", "tail", "less", "more",
];

#[derive(Debug, PartialEq, Eq)]
pub enum NotFoundSuggestion {
    Typo(String),
    Install { formula: String },
    MacOsEquivalent(String),
    Unknown,
}

pub fn suggest_not_found(command: &str, _bank: Option<&Bank>) -> NotFoundSuggestion {
    // 1. Linux→macOS exact match
    if let Some((_, macos)) = LINUX_TO_MACOS.iter().find(|(linux, _)| *linux == command) {
        return NotFoundSuggestion::MacOsEquivalent(macos.to_string());
    }

    // 2. Installable via brew exact match
    if let Some((_, formula)) = BREW_MAP.iter().find(|(tool, _)| *tool == command) {
        return NotFoundSuggestion::Install { formula: formula.to_string() };
    }

    // 3. Typo: levenshtein ≤ 2 against known commands
    let best = KNOWN_COMMANDS
        .iter()
        .map(|c| (*c, levenshtein(command, c)))
        .filter(|(_, d)| *d <= 2)
        .min_by_key(|(_, d)| *d);

    if let Some((candidate, _)) = best {
        return NotFoundSuggestion::Typo(candidate.to_string());
    }

    NotFoundSuggestion::Unknown
}

/// Levenshtein distance between two strings (capped at 3 for performance).
pub fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let (m, n) = (a.len(), b.len());

    if m == 0 { return n; }
    if n == 0 { return m; }

    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0usize; n + 1];

    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            curr[j] = (curr[j - 1] + 1)
                .min(prev[j] + 1)
                .min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[n]
}
