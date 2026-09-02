#[tokio::main]
async fn main() -> anyhow::Result<()> {
    zuckerbot_api::run().await
}
