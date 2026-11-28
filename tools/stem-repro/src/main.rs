mod app;
mod features;
mod shared;

fn main() -> anyhow::Result<()> {
    app::run()
}
