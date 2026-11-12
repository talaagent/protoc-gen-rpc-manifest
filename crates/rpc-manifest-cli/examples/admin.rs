//! A minimal admin CLI embedding the `rpc` subcommand.
//!
//! ```sh
//! cargo run -p rpc-manifest-cli --example admin -- rpc
//! cargo run -p rpc-manifest-cli --example admin -- --url http://localhost:8080 rpc user GetUser '{"id":"7"}'
//! ```

use std::process::ExitCode;

use bpaf::Bpaf;
use rpc_manifest::Manifest;
use rpc_manifest::connect::{self, Client};
use rpc_manifest_cli::RpcArgs;

// In a real CLI: `include!(concat!(env!("OUT_DIR"), "/rpc_manifest.rs"));`
// or a checked-in file written by `buf generate`.
include!("../../protoc-gen-rpc-manifest/tests/golden/rpc_manifest.rs");

#[derive(Debug, Clone, Bpaf)]
#[bpaf(options)]
struct Cli {
    /// Server base URL
    #[bpaf(long, argument("URL"), fallback("http://localhost:8080".to_owned()))]
    url: String,
    /// Bearer token
    #[bpaf(long, env("ADMIN_TOKEN"), argument("TOKEN"))]
    token: Option<String>,
    #[bpaf(external(command))]
    command: Command,
}

#[derive(Debug, Clone, Bpaf)]
enum Command {
    Rpc(#[bpaf(external(rpc_manifest_cli::rpc_command))] RpcArgs),
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = cli().run();
    let Command::Rpc(args) = cli.command;
    let mut client = Client::new(cli.url).timeout(core::time::Duration::from_secs(30));
    if let Some(token) = cli.token.as_deref() {
        match connect::bearer(token) {
            Ok(hook) => client = client.hook(hook),
            Err(e) => {
                eprintln!("error[{}]: {e}", e.code());
                return ExitCode::FAILURE;
            }
        }
    }
    let manifest = Manifest::new(SERVICES);
    let mut stdout = std::io::stdout().lock();
    match rpc_manifest_cli::run(manifest, &client, &args, "admin rpc", &mut stdout).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error[{}]: {e}", e.code());
            ExitCode::FAILURE
        }
    }
}
