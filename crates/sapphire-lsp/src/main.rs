//! `sapphire-lsp` binary entry point (L1 scaffold).
//!
//! Reads LSP JSON-RPC messages from stdin and writes responses to
//! stdout, following the LSP 3.17 transport convention. Logging goes
//! to **stderr** via `tracing-subscriber` so it does not corrupt the
//! stdout transport frame.
//!
//! CLI surface (L1):
//!
//! - No positional arguments: run the server on stdin/stdout.
//! - `--version` / `-V`: print the crate version and exit.
//! - `--help` / `-h`: print a short usage blurb and exit.
//!
//! Other transports (TCP, pipe) are deliberately out of scope for L1;
//! see `I-OQ10` in `docs/open-questions.md`.

use std::io::Write;

use sapphire_lsp::SapphireLanguageServer;
use tower_lsp::{LspService, Server};
use tracing_subscriber::EnvFilter;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn print_help(out: &mut impl Write) -> std::io::Result<()> {
    writeln!(
        out,
        "sapphire-lsp {VERSION}\n\
         \n\
         Language Server for the Sapphire language.\n\
         \n\
         USAGE:\n\
         \x20   sapphire-lsp            speak LSP on stdin/stdout\n\
         \x20   sapphire-lsp --version  print version and exit\n\
         \x20   sapphire-lsp --help     print this help and exit\n\
         \n\
         Logs are written to stderr. Set SAPPHIRE_LSP_LOG to a\n\
         `tracing_subscriber::EnvFilter` expression (e.g. \"debug\")\n\
         to raise verbosity.\n",
    )
}

enum CliAction {
    Serve,
    PrintVersion,
    PrintHelp,
    Unknown(String),
}

fn parse_args(args: impl IntoIterator<Item = String>) -> CliAction {
    let mut iter = args.into_iter();
    // Skip argv[0].
    let _ = iter.next();
    match iter.next() {
        None => CliAction::Serve,
        Some(flag) => match flag.as_str() {
            "--version" | "-V" => CliAction::PrintVersion,
            "--help" | "-h" => CliAction::PrintHelp,
            _ => CliAction::Unknown(flag),
        },
    }
}

fn init_tracing() {
    // Default to INFO level; override via SAPPHIRE_LSP_LOG.
    let filter =
        EnvFilter::try_from_env("SAPPHIRE_LSP_LOG").unwrap_or_else(|_| EnvFilter::new("info"));
    // `with_writer(std::io::stderr)` keeps LSP stdout clean.
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    match parse_args(std::env::args()) {
        CliAction::PrintVersion => {
            println!("sapphire-lsp {VERSION}");
            return;
        }
        CliAction::PrintHelp => {
            let mut stdout = std::io::stdout().lock();
            print_help(&mut stdout).expect("write help to stdout");
            return;
        }
        CliAction::Unknown(flag) => {
            let mut stderr = std::io::stderr().lock();
            let _ = writeln!(stderr, "sapphire-lsp: unknown argument: {flag}");
            let _ = print_help(&mut stderr);
            std::process::exit(2);
        }
        CliAction::Serve => {}
    }

    init_tracing();
    tracing::info!(version = VERSION, "sapphire-lsp starting");

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(SapphireLanguageServer::new);
    Server::new(stdin, stdout, socket).serve(service).await;

    tracing::info!("sapphire-lsp stopped");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn parse_args_no_flags_serves() {
        assert!(matches!(
            parse_args(args(&["sapphire-lsp"])),
            CliAction::Serve
        ));
    }

    #[test]
    fn parse_args_version() {
        assert!(matches!(
            parse_args(args(&["sapphire-lsp", "--version"])),
            CliAction::PrintVersion
        ));
        assert!(matches!(
            parse_args(args(&["sapphire-lsp", "-V"])),
            CliAction::PrintVersion
        ));
    }

    #[test]
    fn parse_args_help() {
        assert!(matches!(
            parse_args(args(&["sapphire-lsp", "--help"])),
            CliAction::PrintHelp
        ));
        assert!(matches!(
            parse_args(args(&["sapphire-lsp", "-h"])),
            CliAction::PrintHelp
        ));
    }

    #[test]
    fn parse_args_unknown() {
        assert!(matches!(
            parse_args(args(&["sapphire-lsp", "--nope"])),
            CliAction::Unknown(_)
        ));
    }
}
