//! `sbol`: command-line tool for SBOL 3 documents.
//!
//! See `sbol validate --help` for the full surface.

use std::process::ExitCode;

use clap::Parser;

mod cli;
mod commands;
mod output;
mod style;

mod sarif;

use cli::{Cli, Command};
use style::Styles;

fn main() -> ExitCode {
    let cli = Cli::parse();
    let styles = Styles::resolve(cli.color);
    match cli.command {
        Command::Init(args) => commands::workspace::init(args, styles),
        Command::Status(args) => commands::workspace::status(args, styles),
        Command::Sync(args) => commands::workspace::sync(args, styles),
        Command::Validate(args) => commands::validate(args, styles),
        Command::Diff(args) => commands::diff(args, styles),
        Command::Convert(args) => commands::convert(args, styles),
        Command::Upgrade(args) => commands::upgrade(args, styles),
        Command::Downgrade(args) => commands::downgrade(args, styles),
        Command::ImportGenbank(args) => commands::import_genbank(args, styles),
        Command::ImportFasta(args) => commands::import_fasta(args, styles),
        Command::Rules(command) => commands::rules(command, styles),
        Command::Ontology(command) => commands::ontology(command, styles),
        Command::Registry(command) => commands::registry(command, styles),
    }
}
