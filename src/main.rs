#![recursion_limit = "256"]
#![forbid(unsafe_code)]

mod agent;
mod cli;
mod doctor;
mod plan;
mod report;
mod runner;
mod schema;
mod skills;
mod state;
mod templates;
mod util;
mod worktree;

fn main() {
    if let Err(error) = cli::run() {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}
