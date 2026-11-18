//! Brachio terminal runtime and its local client.

mod journal;
mod output;
mod screen;
mod service;

fn main() -> anyhow::Result<()> {
    service::run()
}
