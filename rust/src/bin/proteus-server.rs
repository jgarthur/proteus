use std::error::Error;

use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let bind_addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:3000".to_owned());

    let listener = TcpListener::bind(&bind_addr).await?;
    eprintln!("proteus server listening on {bind_addr}");
    proteus::web::serve(listener).await?;
    Ok(())
}
