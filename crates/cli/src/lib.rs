mod broker;
mod cli;
mod commands;
mod output;
mod update;

#[doc(hidden)]
pub mod test {
    pub use crate::broker::__test as broker;
    pub use crate::cli::__test as cli;
    pub use crate::update::__test as update;
}

pub use cli::{channel, help, version};

pub fn run(args: Vec<String>) -> Result<(), String> {
    cli::run(args)
}
