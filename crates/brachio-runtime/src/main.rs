//! Brachio terminal runtime and its local client.

mod endpoint;
mod journal;
mod output;
mod protocol;
mod screen;
mod service;
mod transport;

fn main() -> anyhow::Result<()> {
    service::run()
}
