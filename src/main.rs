use quasimodo::{CliArgs, OllamaAdapter, run};

fn main() {
    let args = match CliArgs::parse(std::env::args().skip(1)) {
        Ok(a) => a,
        Err(msg) => {
            eprintln!("error: {msg}");
            eprintln!(
                "usage: quasimodo (--prompt <text> | --notfound <cmd> | --explain <context>) [--model <name>] [--endpoint <url>] [--bank <path>] [--samples <n>] [--temperature <f>]"
            );
            std::process::exit(1);
        }
    };

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
