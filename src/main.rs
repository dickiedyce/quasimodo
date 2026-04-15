use quasimodo::{CliArgs, OllamaAdapter, run, usage_text};

fn main() {
    let mut args = match CliArgs::parse(std::env::args().skip(1)) {
        Ok(a) => a,
        Err(msg) => {
            eprintln!("error: {msg}");
            eprintln!("{}", usage_text());
            std::process::exit(1);
        }
    };

    if args.help {
        println!("{}", usage_text());
        return;
    }

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
