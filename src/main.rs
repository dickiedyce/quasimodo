use quasimodo::{CliArgs, OllamaAdapter, run};

fn main() {
    let mut args = match CliArgs::parse(std::env::args().skip(1)) {
        Ok(a) => a,
        Err(msg) => {
            eprintln!("error: {msg}");
            eprintln!(
                "usage: quasimodo (--prompt <text> | --stdin | --notfound <cmd> | --explain <context> | --describe <cmd> | --teach <text> --command <cmd> | --list-taught | --delete-taught <text>) [--model <name>] [--endpoint <url>] [--bank <path>] [--samples <n>] [--temperature <f>] [--system <text>] [--history-file <path>] [--no-quality-retry]"
            );
            std::process::exit(1);
        }
    };

    if args.stdin {
        use std::io::Read;

        let mut buf = String::new();
        if std::io::stdin().read_to_string(&mut buf).is_ok() {
            args.prompt = buf.trim().to_string();
        }
        if args.prompt.is_empty() {
            eprintln!("error: --stdin provided but no input was read");
            std::process::exit(1);
        }
    }

    let adapter = match OllamaAdapter::new(&args.endpoint) {
        Ok(a) => a,
        Err(err) => {
            eprintln!("error: invalid adapter configuration: {err:?}");
            std::process::exit(1);
        }
    };

    match run(&args, &adapter) {
        Ok(text) => println!("{text}"),
        Err(err) => {
            eprintln!("error: {err:?}");
            std::process::exit(1);
        }
    }
}
