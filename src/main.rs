mod create;
mod get;
mod root;
mod scan;
mod selector;
mod spec;
mod tty;

use std::env;
use std::process::ExitCode;

/// 0: success / 1: aborted or nothing found / 2: error
const EXIT_OK: u8 = 0;
const EXIT_NONE: u8 = 1;
const EXIT_ERROR: u8 = 2;

const USAGE: &str = "\
gh yard — pick a repository and print its path

Usage:
  gh yard                        open the selector and print the chosen repository's absolute path
  gh yard list [-p|--full-path]  print repositories, one per line
  gh yard get <spec>             clone and print the path
  gh yard create <spec>          create a local repository (git init) and print the path
  gh yard root                   print the root directory

Examples:
  set -l d (gh yard); and cd $d          # fish
  d=$(gh yard) && cd \"$d\"                # zsh / bash
";

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let code = match run(&args) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("gh yard: {message}");
            EXIT_ERROR
        }
    };
    ExitCode::from(code)
}

fn run(args: &[String]) -> Result<u8, String> {
    match args.first().map(String::as_str) {
        None => select(),
        Some("list") => list(&args[1..]),
        Some("get") => get(&args[1..]),
        Some("create") => create(&args[1..]),
        Some("root") => {
            println!("{}", root::resolve()?.display());
            Ok(EXIT_OK)
        }
        Some("-h" | "--help" | "help") => {
            print!("{USAGE}");
            Ok(EXIT_OK)
        }
        Some("--version" | "-V") => {
            println!("gh-yard {}", env!("CARGO_PKG_VERSION"));
            Ok(EXIT_OK)
        }
        Some(other) => Err(format!("unknown subcommand: {other}\n\n{USAGE}")),
    }
}

fn select() -> Result<u8, String> {
    let root = root::resolve()?;
    let repos = scan::scan(&root);
    if repos.is_empty() {
        eprintln!("gh yard: no repositories under {}", root.display());
        return Ok(EXIT_NONE);
    }

    match selector::run(&repos)? {
        selector::Outcome::Selected(path) => {
            println!("{path}");
            Ok(EXIT_OK)
        }
        selector::Outcome::Aborted => Ok(EXIT_NONE),
    }
}

fn list(args: &[String]) -> Result<u8, String> {
    let mut full_path = false;
    for arg in args {
        match arg.as_str() {
            "-p" | "--full-path" => full_path = true,
            other => return Err(format!("unknown flag for list: {other}")),
        }
    }

    let root = root::resolve()?;
    for repo in scan::scan(&root) {
        if full_path {
            println!("{}", repo.abs.display());
        } else {
            println!("{}", repo.rel);
        }
    }
    Ok(EXIT_OK)
}

fn get(args: &[String]) -> Result<u8, String> {
    let spec = args.first().ok_or("get requires a spec")?;
    if args.len() > 1 {
        return Err("get takes exactly one spec".to_string());
    }

    let target = spec::parse(spec)?;
    let root = root::resolve()?;
    let dest = get::run(&root, &target)?;
    println!("{}", dest.display());
    Ok(EXIT_OK)
}

fn create(args: &[String]) -> Result<u8, String> {
    let spec = args.first().ok_or("create requires a spec")?;
    if args.len() > 1 {
        return Err("create takes exactly one spec".to_string());
    }

    let target = spec::parse(spec)?;
    let root = root::resolve()?;
    let dest = create::run(&root, &target)?;
    println!("{}", dest.display());
    Ok(EXIT_OK)
}
