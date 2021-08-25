#[async_std::main]
async fn main() -> surf_pool::Result<()> {
    let builder = surf_pool::SurfPoolBuilder::new(3)
        .unwrap()
        .health_check(surf::get("https://pot.pizzamig.dev"))
        .pre_connect(false);
    let uut = builder.build().await;
    let handler = uut.get_handler().await.expect("This shouldn't happen");
    handler
        .get_client()
        .get("https://pot.pizzamig.dev")
        .recv_string()
        .await
        .expect("Error while receiving data - first request");
    drop(handler);
    let handler = uut.get_handler().await.expect("This shouldn't happen");
    handler
        .get_client()
        .get("https://pot.pizzamig.dev")
        .recv_string()
        .await
        .expect("Error while receiving data - first request");
    Ok(())
}
