#[tokio::main]
async fn main() -> anyhow::Result<()> {
    bscp_userserver::run().await
}
