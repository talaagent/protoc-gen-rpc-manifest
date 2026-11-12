//! The `rpc` subcommand end to end on the plugin's golden manifest.

use axum::Router;
use axum::extract::Path;
use rpc_manifest::Manifest;
use rpc_manifest::connect::Client;
use rpc_manifest_cli::{Action, RpcArgs};
use serde_json::{Value, json};

include!("../../protoc-gen-rpc-manifest/tests/golden/rpc_manifest.rs");

const MANIFEST: Manifest = Manifest::new(SERVICES);

fn args(parts: &[&str]) -> RpcArgs {
    let get = |i: usize| parts.get(i).map(|s| (*s).to_owned());
    RpcArgs {
        service: get(0),
        method: get(1),
        body: get(2),
    }
}

async fn output(parts: &[&str], client: &Client) -> Result<String, rpc_manifest_cli::Error> {
    let mut out = Vec::new();
    rpc_manifest_cli::run(MANIFEST, client, &args(parts), "adm rpc", &mut out).await?;
    Ok(String::from_utf8(out).expect("utf-8"))
}

fn offline() -> Client {
    Client::new("http://127.0.0.1:9")
}

#[tokio::test]
async fn lists_services() {
    let out = output(&[], &offline()).await.expect("list");
    assert_eq!(
        out,
        "\
http-invoice  acme.billing.v1.HTTPInvoiceService  Block comments work too.
uadmin        acme.user.v1.UserAdminService       Second-tier user admin; its alias comes from the plugin options.
user          acme.user.v1.UserService            Manages user accounts.

List a service's methods: adm rpc <SERVICE>
"
    );
}

#[tokio::test]
async fn lists_methods() {
    let out = output(&["USER"], &offline()).await.expect("methods");
    assert_eq!(
        out,
        "\
acme.user.v1.UserService (user)

Manages user accounts.

Every method needs an admin token.

  GetUser     Fetches one user by id.
  ListUsers   Lists users, newest first.
  RenameUser  Sets a user's \"display name\" (quotes, \\backslash and ünïcode survive).
  DeleteUser
  WatchUsers  Streams changes to users as they happen. [server streaming]

Show a method's docs: adm rpc user <METHOD>
"
    );
}

#[tokio::test]
async fn describes_a_method() {
    let out = output(&["user", "list_users"], &offline())
        .await
        .expect("describe");
    assert_eq!(
        out,
        "\
acme.user.v1.UserService/ListUsers
  request:     acme.common.v1.PageRequest
  response:    acme.user.v1.ListUsersResponse
  idempotency: no side effects
  streaming:   unary

Lists users, newest first.

Call it: adm rpc user ListUsers '<JSON>'
"
    );
}

#[tokio::test]
async fn bad_names_suggest() {
    let err = output(&["usr"], &offline()).await.unwrap_err();
    assert_eq!(err.code(), "unknown_service");
    assert_eq!(
        err.to_string(),
        "unknown service `usr`; did you mean `user`?"
    );

    let err = output(&["user", "GetUsr"], &offline()).await.unwrap_err();
    assert_eq!(err.code(), "unknown_method");
    assert_eq!(
        err.to_string(),
        "unknown method `GetUsr` on `acme.user.v1.UserService`; did you mean `GetUser`?"
    );
}

#[tokio::test]
async fn validates_body_before_calling() {
    let err = output(&["user", "GetUser", "{nope"], &offline())
        .await
        .unwrap_err();
    assert_eq!(err.code(), "invalid_json");
    let err = output(&["user", "GetUser", "@/nonexistent/req.json"], &offline())
        .await
        .unwrap_err();
    assert_eq!(err.code(), "read_body");
    let err = output(&["user", "WatchUsers", "{}"], &offline())
        .await
        .unwrap_err();
    assert_eq!(err.code(), "streaming_unsupported");
}

#[test]
fn only_calls_need_a_client() {
    let action = args(&["user"]).resolve(MANIFEST).expect("resolve");
    assert!(matches!(action, Action::ListMethods(s) if s.alias == "user"));
    assert!(!action.needs_client());
    let action = args(&["user", "GetUser", "{}"])
        .resolve(MANIFEST)
        .expect("resolve");
    assert!(action.needs_client());
}

#[tokio::test]
async fn calls_and_pretty_prints() {
    async fn handler(
        Path((service, method)): Path<(String, String)>,
        body: String,
    ) -> axum::Json<Value> {
        let req: Value = serde_json::from_str(&body).expect("json");
        axum::Json(json!({"path": format!("{service}/{method}"), "req": req}))
    }
    let app = Router::new().route("/{service}/{method}", axum::routing::post(handler));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let base = format!("http://{}", listener.local_addr().expect("addr"));
    tokio::spawn(async move { axum::serve(listener, app).await });

    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("req.json");
    std::fs::write(&file, r#"{"id": "7"}"#).expect("write");
    let at_file = format!("@{}", file.display());

    let out = output(&["user", "get-user", &at_file], &Client::new(base))
        .await
        .expect("call");
    assert_eq!(
        out,
        "\
{
  \"path\": \"acme.user.v1.UserService/GetUser\",
  \"req\": {
    \"id\": \"7\"
  }
}
"
    );
}

#[cfg(feature = "bpaf")]
#[test]
fn bpaf_parses() {
    use bpaf::{Parser as _, construct};

    #[derive(Debug, Clone)]
    enum Cmd {
        Rpc(RpcArgs),
        Other,
    }
    let rpc = rpc_manifest_cli::rpc_command().map(Cmd::Rpc);
    let other = bpaf::pure(Cmd::Other).to_options().command("other");
    let parser = construct!([rpc, other]).to_options();
    let parse = |argv: &[&str]| match parser.run_inner(argv).expect("parse") {
        Cmd::Rpc(a) => a,
        Cmd::Other => panic!("parsed `other`"),
    };
    assert_eq!(parse(&["rpc"]), RpcArgs::default());
    assert_eq!(
        parse(&["rpc", "user", "GetUser", "-"]),
        args(&["user", "GetUser", "-"])
    );
    assert_eq!(
        parse(&["rpc", "user", "GetUser", r#"{"id":"7"}"#]),
        args(&["user", "GetUser", r#"{"id":"7"}"#])
    );
}

#[cfg(feature = "clap")]
#[test]
fn clap_parses() {
    use clap::Parser as _;

    #[derive(clap::Parser)]
    struct Cli {
        #[command(subcommand)]
        cmd: Cmd,
    }
    #[derive(clap::Subcommand)]
    enum Cmd {
        /// Call any RPC by service and method name
        Rpc(RpcArgs),
    }
    let parse = |argv: &[&str]| {
        let Cmd::Rpc(a) = Cli::try_parse_from(argv).expect("parse").cmd;
        a
    };
    assert_eq!(parse(&["adm", "rpc"]), RpcArgs::default());
    assert_eq!(
        parse(&["adm", "rpc", "user", "GetUser", "-"]),
        args(&["user", "GetUser", "-"])
    );
}
