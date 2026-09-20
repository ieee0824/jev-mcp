mod client;
mod error;
mod server;
mod types;

#[cfg(test)]
mod tests;

use rmcp::ServiceExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = client::TypeSafeClient::from_env().map_err(std::io::Error::other)?;
    let service = server::JevServer::new(client)
        .serve(rmcp::transport::stdio())
        .await?;
    service.waiting().await?;
    Ok(())
}
