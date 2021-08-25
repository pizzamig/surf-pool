#[async_std::main]
async fn main() -> surf_pool::Result<()> {
    let builder = surf_pool::SurfPoolBuilder::new(3)
        .unwrap()
        .health_check(surf::get("https://pot.pizzamig.dev"))
        .pre_connect(true);
    let uut = builder.build().await;
    let handler = uut
        .get_handler()
        .await
        .expect("This is not supposed to happen");
    handler
        .get_client()
        .get("https://pot.pizzamig.dev")
        .recv_string()
        .await
        .expect("Failed to receive data");
    Ok(())
}
