mod agent;
mod cli;
mod plan;
mod report;
mod runner;
mod schema;
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
