mod broker_runtime;
mod cli;
mod commands;
mod output;
mod update;

#[doc(hidden)]
pub use broker_runtime::__test as broker_runtime_test;
#[doc(hidden)]
pub use cli::__test as cli_test;
pub use cli::{channel, help_text, version};
#[doc(hidden)]
pub use update::__test as update_test;

pub fn run(args: Vec<String>) -> Result<(), String> {
    cli::run(args)
}
