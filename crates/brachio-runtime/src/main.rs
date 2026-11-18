//! Brachio terminal runtime and its local client.

mod cli;
mod journal;
mod launcher;
mod output;
mod recovery;
mod runtime;
mod screen;
mod server;
mod session;

fn main() -> anyhow::Result<()> {
    cli::run()
}
