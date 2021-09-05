#[async_std::main]
async fn main() -> surf_pool::Result<()> {
    let builder = surf_pool::SurfPoolBuilder::new(3)
        .unwrap()
        .health_check(surf::get("https://pot.pizzamig.dev"))
        .pre_connect(true);
    let pool = builder.build().await;
    let handler = pool.get_handler().await;
    handler
        .get_client()
        .get("https://pot.pizzamig.dev")
        .recv_string()
        .await
        .expect("Failed to receive data");
    Ok(())
}
