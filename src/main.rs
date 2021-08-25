mod commands;

use crate::commands::help::HelpCommand;
use crate::commands::install::InstallCommand;
use crate::commands::latest::LatestCommand;
use crate::commands::list::ListCommand;
use commands::list::ListAllCommand;
use structopt::{paw, StructOpt};

#[derive(StructOpt, Debug)]
pub enum Command {
    /// Output documentation for plugin and tool
    Help(HelpCommand),
    /// Install package versions
    Install(InstallCommand),
    Latest(LatestCommand),
    List(ListCommand),
    ListAll(ListAllCommand),
}

#[paw::main]
fn main(args: Command) {
    if let Err(e) = match args {
        Command::Help(command) => command.run(),
        Command::Install(command) => command.run(),
        Command::Latest(command) => command.run(),
        Command::List(command) => command.run(),
        Command::ListAll(command) => command.run(),
    } {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}
