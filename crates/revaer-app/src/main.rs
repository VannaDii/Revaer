use anyhow::Result;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    revaer_telemetry::init_tracing();

    info!("Revaer application bootstrap starting");
    // TODO: wire configuration bootstrap, migrations, and module activation here.

    Ok(())
}
