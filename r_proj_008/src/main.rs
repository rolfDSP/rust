//! Command line options example using the `clap` crate (derive API).
//!
//! The whole command line interface is described by plain Rust structs and
//! enums. `clap` generates the parser, the `--help` / `--version` output and
//! error messages from them. Doc comments (`///`) become the help texts.
//!
//! Try for example:
//!   cargo run -- --help
//!   cargo run -- greet --help
//!   cargo run -- greet Rolf --times 3 --lang de --shout
//!   cargo run -- -vv calc mul 6 7
//!   cargo run -- stats Cargo.toml src/main.rs --lines --words
//!   echo "hello world" | cargo run -- stats

use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

/// A small demo tool showing different kinds of command line options.
#[derive(Parser, Debug)]
#[command(version, about)]
struct Cli {
    /// Increase output detail (-v, -vv, -vvv)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    verbose: u8,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Greet somebody
    Greet(GreetArgs),
    /// Do a simple calculation
    Calc(CalcArgs),
    /// Count lines, words and characters of files or stdin
    Stats(StatsArgs),
}

#[derive(Args, Debug)]
struct GreetArgs {
    /// Name of the person to greet (positional argument)
    name: String,

    /// How often to repeat the greeting
    #[arg(short, long, default_value_t = 1, value_parser = clap::value_parser!(u8).range(1..=10))]
    times: u8,

    /// Language of the greeting (falls back to the GREET_LANG environment variable)
    #[arg(short, long, value_enum, env = "GREET_LANG", default_value_t = Language::En)]
    lang: Language,

    /// Print the greeting in upper case
    #[arg(short, long)]
    shout: bool,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq)]
enum Language {
    En,
    De,
    Fr,
}

#[derive(Args, Debug)]
struct CalcArgs {
    /// Operation to perform
    #[arg(value_enum)]
    op: Operation,
    /// First operand
    a: f64,
    /// Second operand
    b: f64,

    /// Number of decimal places in the result
    #[arg(short, long, default_value_t = 2)]
    precision: usize,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq)]
enum Operation {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Args, Debug)]
struct StatsArgs {
    /// Files to analyse; reads from stdin if none are given
    files: Vec<PathBuf>,

    /// Show line count
    #[arg(short, long)]
    lines: bool,
    /// Show word count
    #[arg(short, long)]
    words: bool,
    /// Show character count
    #[arg(short, long)]
    chars: bool,
}

fn main() {
    let cli = Cli::parse();

    if cli.verbose >= 2 {
        println!("Parsed arguments: {cli:#?}\n");
    }

    let result = match cli.command {
        Command::Greet(args) => {
            greet(&args);
            Ok(())
        }
        Command::Calc(args) => calc(&args, cli.verbose),
        Command::Stats(args) => stats(&args),
    };

    if let Err(message) = result {
        eprintln!("error: {message}");
        std::process::exit(1);
    }
}

fn greet(args: &GreetArgs) {
    let word = match args.lang {
        Language::En => "Hello",
        Language::De => "Hallo",
        Language::Fr => "Bonjour",
    };
    let mut text = format!("{word}, {}!", args.name);
    if args.shout {
        text = text.to_uppercase();
    }
    for _ in 0..args.times {
        println!("{text}");
    }
}

fn calc(args: &CalcArgs, verbose: u8) -> Result<(), String> {
    let (symbol, result) = match args.op {
        Operation::Add => ("+", args.a + args.b),
        Operation::Sub => ("-", args.a - args.b),
        Operation::Mul => ("*", args.a * args.b),
        Operation::Div if args.b == 0.0 => return Err("division by zero".into()),
        Operation::Div => ("/", args.a / args.b),
    };
    let p = args.precision;
    if verbose >= 1 {
        println!("{} {symbol} {} = {result:.p$}", args.a, args.b);
    } else {
        println!("{result:.p$}");
    }
    Ok(())
}

fn stats(args: &StatsArgs) -> Result<(), String> {
    // Without any selection flag, show everything (like `wc`).
    let show_all = !(args.lines || args.words || args.chars);

    let inputs: Vec<(String, String)> = if args.files.is_empty() {
        let mut text = String::new();
        io::stdin()
            .read_to_string(&mut text)
            .map_err(|e| format!("cannot read stdin: {e}"))?;
        vec![("<stdin>".into(), text)]
    } else {
        args.files
            .iter()
            .map(|path| {
                fs::read_to_string(path)
                    .map(|text| (path.display().to_string(), text))
                    .map_err(|e| format!("cannot read {}: {e}", path.display()))
            })
            .collect::<Result<_, _>>()?
    };

    for (name, text) in inputs {
        let mut parts = Vec::new();
        if show_all || args.lines {
            parts.push(format!("{:>6} lines", text.lines().count()));
        }
        if show_all || args.words {
            parts.push(format!("{:>6} words", text.split_whitespace().count()));
        }
        if show_all || args.chars {
            parts.push(format!("{:>6} chars", text.chars().count()));
        }
        println!("{}  {name}", parts.join(" "));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn parses_greet_with_options() {
        let cli = Cli::try_parse_from(["app", "greet", "Rolf", "-t", "3", "--lang", "de", "-s"]).unwrap();
        let Command::Greet(args) = cli.command else { panic!("expected greet") };
        assert_eq!(args.name, "Rolf");
        assert_eq!(args.times, 3);
        assert_eq!(args.lang, Language::De);
        assert!(args.shout);
    }

    #[test]
    fn counts_global_verbose_flag() {
        let cli = Cli::try_parse_from(["app", "calc", "add", "1", "2", "-vvv"]).unwrap();
        assert_eq!(cli.verbose, 3);
    }

    #[test]
    fn rejects_out_of_range_value() {
        assert!(Cli::try_parse_from(["app", "greet", "Rolf", "--times", "11"]).is_err());
    }

    #[test]
    fn rejects_unknown_operation() {
        assert!(Cli::try_parse_from(["app", "calc", "pow", "2", "3"]).is_err());
    }
}
