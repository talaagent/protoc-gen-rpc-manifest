//! protoc/buf plugin entry point: `CodeGeneratorRequest` on stdin,
//! `CodeGeneratorResponse` on stdout. Option errors go back in the
//! response's `error` field, as the plugin protocol asks.

use std::io::{Read, Write};
use std::process::ExitCode;

use prost::Message;
use prost_types::compiler::CodeGeneratorRequest;

fn main() -> ExitCode {
    let mut input = Vec::new();
    if let Err(e) = std::io::stdin().read_to_end(&mut input) {
        eprintln!("protoc-gen-rpc-manifest: reading stdin: {e}");
        return ExitCode::FAILURE;
    }
    let request = match CodeGeneratorRequest::decode(input.as_slice()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("protoc-gen-rpc-manifest: decoding CodeGeneratorRequest: {e}");
            return ExitCode::FAILURE;
        }
    };
    let response = protoc_gen_rpc_manifest::respond(&request);
    if let Err(e) = std::io::stdout().write_all(&response.encode_to_vec()) {
        eprintln!("protoc-gen-rpc-manifest: writing stdout: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
