//! The caller against a tiny in-test Connect JSON server.

use core::time::Duration;

use axum::Router;
use axum::extract::Path;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use rpc_manifest::connect::{self, CallError, Client, Code};
use rpc_manifest::{Idempotency, Method, Service, Streaming};
use serde_json::{Value, json};

async fn handler(
    Path((service, method)): Path<(String, String)>,
    headers: HeaderMap,
    body: String,
) -> Response {
    let header = |name: &str| headers.get(name).and_then(|v| v.to_str().ok());
    assert_eq!(header("content-type"), Some("application/json"));
    assert_eq!(header("connect-protocol-version"), Some("1"));
    let error = |status: StatusCode, v: Value| (status, axum::Json(v)).into_response();
    if header("authorization") != Some("Bearer s3cret") {
        return error(
            StatusCode::UNAUTHORIZED,
            json!({"code": "unauthenticated", "message": "no token"}),
        );
    }
    match (service.as_str(), method.as_str()) {
        ("acme.echo.v1.EchoService", "Echo") => {
            let req: Value = serde_json::from_str(&body).expect("json body");
            axum::Json(json!({
                "echo": req,
                "timeout": header("connect-timeout-ms"),
                "key": header("x-api-key"),
            }))
            .into_response()
        }
        ("acme.echo.v1.EchoService", "Fail") => error(
            StatusCode::BAD_REQUEST,
            json!({
                "code": "invalid_argument",
                "message": "bad name",
                "details": [{"type": "google.rpc.BadRequest", "value": "CgA", "debug": {"field": "name"}}],
            }),
        ),
        ("acme.echo.v1.EchoService", "Slow") => {
            tokio::time::sleep(Duration::from_secs(5)).await;
            axum::Json(json!({})).into_response()
        }
        ("acme.echo.v1.EchoService", "Gateway") => {
            (StatusCode::BAD_GATEWAY, "upstream gone").into_response()
        }
        _ => error(
            StatusCode::NOT_FOUND,
            json!({"code": "unimplemented", "message": format!("{service}/{method}")}),
        ),
    }
}

async fn serve() -> String {
    let app = Router::new().route("/{service}/{method}", axum::routing::post(handler));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move { axum::serve(listener, app).await });
    format!("http://{addr}/")
}

async fn client() -> Client {
    Client::new(serve().await).hook(connect::bearer("s3cret").expect("token"))
}

#[tokio::test]
async fn unary_round_trip() {
    let c = client()
        .await
        .hook(connect::header("x-api-key", "k1").expect("header"))
        .timeout(Duration::from_secs(3));
    let res: Value = c
        .call_json("acme.echo.v1.EchoService", "Echo", &json!({"msg": "hi"}))
        .await
        .expect("call");
    assert_eq!(
        res,
        json!({"echo": {"msg": "hi"}, "timeout": "3000", "key": "k1"})
    );
    // Raw bytes in, raw bytes out.
    let raw = c
        .call("acme.echo.v1.EchoService", "Echo", "[1,2]")
        .await
        .expect("raw call");
    let v: Value = serde_json::from_slice(&raw).expect("json");
    assert_eq!(v["echo"], json!([1, 2]));
}

#[tokio::test]
async fn decodes_connect_errors() {
    let c = client().await;
    let err = c
        .call("acme.echo.v1.EchoService", "Fail", "{}")
        .await
        .unwrap_err();
    assert_eq!(err.code(), "invalid_argument");
    let CallError::Connect(e) = err else {
        panic!("want a Connect error, got {err:?}")
    };
    assert_eq!(
        (e.code, e.message.as_str(), e.http_status),
        (Code::InvalidArgument, "bad name", 400)
    );
    assert_eq!(e.details[0].type_name, "google.rpc.BadRequest");
    assert_eq!(e.details[0].debug, Some(json!({"field": "name"})));

    let err = c
        .call("acme.echo.v1.EchoService", "Nope", "{}")
        .await
        .unwrap_err();
    assert_eq!(err.code(), "unimplemented");

    let err = c
        .call("acme.echo.v1.EchoService", "Gateway", "{}")
        .await
        .unwrap_err();
    assert_eq!(err.code(), "unavailable");
    assert_eq!(err.to_string(), "unavailable: upstream gone");

    let anon = Client::new(c.base());
    let err = anon
        .call("acme.echo.v1.EchoService", "Echo", "{}")
        .await
        .unwrap_err();
    assert_eq!(err.code(), "unauthenticated");
}

#[tokio::test]
async fn times_out_and_reports_transport_errors() {
    let c = client().await.timeout(Duration::from_millis(100));
    let err = c
        .call("acme.echo.v1.EchoService", "Slow", "{}")
        .await
        .unwrap_err();
    assert_eq!(err.code(), "timeout");

    // Nothing listens on the discard port locally.
    let down = Client::new("http://127.0.0.1:9");
    let err = down.call("x.S", "M", "{}").await.unwrap_err();
    assert_eq!(err.code(), "transport");
}

#[tokio::test]
async fn refuses_streaming_methods() {
    static METHOD: Method = Method {
        name: "Watch",
        input_type: "acme.echo.v1.WatchRequest",
        output_type: "acme.echo.v1.WatchResponse",
        idempotency: Idempotency::NoSideEffects,
        streaming: Streaming::Server,
        comments: "",
    };
    static SERVICE: Service = Service {
        full_name: "acme.echo.v1.EchoService",
        package: "acme.echo.v1",
        name: "EchoService",
        alias: "echo",
        comments: "",
        methods: core::slice::from_ref(&METHOD),
    };
    let c = client().await;
    let err = c.call_method(&SERVICE, &METHOD, "{}").await.unwrap_err();
    assert_eq!(err.code(), "streaming_unsupported");
    assert_eq!(
        err.to_string(),
        "`acme.echo.v1.EchoService/Watch` is a server-streaming method; only unary methods can be called"
    );
}
