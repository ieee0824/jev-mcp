mod client;
mod error;
mod eval;
mod server;
mod telemetry;
mod types;

#[cfg(test)]
mod tests;

use rmcp::ServiceExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    if let Some(command) = args.next() {
        let subcommand = args.next();
        if command == "eval" && subcommand.as_deref() == Some(std::ffi::OsStr::new("validate")) {
            let path = args
                .next()
                .ok_or("usage: jev-mcp eval validate <JSONL_PATH>")?;
            if args.next().is_some() {
                return Err("usage: jev-mcp eval validate <JSONL_PATH>".into());
            }
            let count =
                eval::validate_file(std::path::Path::new(&path)).map_err(std::io::Error::other)?;
            println!("validated {count} evaluation cases");
            return Ok(());
        }
        if command == "eval" && subcommand.as_deref() == Some(std::ffi::OsStr::new("run")) {
            let path = args.next().ok_or("usage: jev-mcp eval run <JSONL_PATH>")?;
            if args.next().is_some() {
                return Err("usage: jev-mcp eval run <JSONL_PATH>".into());
            }
            let client = client::TypeSafeClient::from_env().map_err(std::io::Error::other)?;
            eval::run_file(std::path::Path::new(&path), &client, std::io::stdout())
                .await
                .map_err(std::io::Error::other)?;
            return Ok(());
        }
        return Err("usage: jev-mcp [eval validate|run <JSONL_PATH>]".into());
    }
    let client = client::TypeSafeClient::from_env().map_err(std::io::Error::other)?;
    let telemetry = telemetry::Telemetry::from_env().map_err(std::io::Error::other)?;
    let service = server::JevServer::with_telemetry(client, telemetry)
        .serve(rmcp::transport::stdio())
        .await?;
    service.waiting().await?;
    Ok(())
}
