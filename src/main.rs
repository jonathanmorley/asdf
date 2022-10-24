mod commands;

use crate::commands::{
    help::HelpCommand, install::InstallCommand, latest::LatestCommand, list::ListAllCommand,
    list::ListCommand, reshim::ReshimCommand,
};
use commands::current::CurrentCommand;
use structopt::{paw, StructOpt};

#[derive(StructOpt, Debug)]
pub enum Command {
    /// Display current version set or being used
    Current(CurrentCommand),
    /// Output documentation for plugin and tool
    Help(HelpCommand),
    /// Install package versions
    Install(InstallCommand),
    /// Show latest stable version of a package
    Latest(LatestCommand),
    /// List installed versions of a package
    List(ListCommand),
    /// List all versions of a package and optionally filter the returned versions
    ListAll(ListAllCommand),
    /// Recreate shims for version of a package
    Reshim(ReshimCommand),
}

#[paw::main]
fn main(args: Command) {
    if let Err(e) = match args {
        Command::Current(command) => command.run(),
        Command::Help(command) => command.run(),
        Command::Install(command) => command.run(),
        Command::Latest(command) => command.run(),
        Command::List(command) => command.run(),
        Command::ListAll(command) => command.run(),
        Command::Reshim(command) => command.run(),
    } {
        match e.to_string().as_str() {
            "No plugin version set" => {
                std::process::exit(126);
            },
            _ => {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
    }
}
