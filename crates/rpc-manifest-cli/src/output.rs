//! Human output for listings, docs and responses.

use std::io::{self, Write};

use rpc_manifest::{Idempotency, Method, Service, Streaming};

pub fn services(services: &[Service], prog: &str, out: &mut impl Write) -> io::Result<()> {
    if services.is_empty() {
        return writeln!(out, "no services");
    }
    let alias_w = services.iter().map(|s| s.alias.len()).max().unwrap_or(0);
    let name_w = services
        .iter()
        .map(|s| s.full_name.len())
        .max()
        .unwrap_or(0);
    for s in services {
        let line = format!(
            "{:alias_w$}  {:name_w$}  {}",
            s.alias,
            s.full_name,
            s.summary()
        );
        writeln!(out, "{}", line.trim_end())?;
    }
    writeln!(out, "\nList a service's methods: {prog} <SERVICE>")
}

pub fn methods(s: &Service, prog: &str, out: &mut impl Write) -> io::Result<()> {
    writeln!(out, "{} ({})", s.full_name, s.alias)?;
    if !s.comments.is_empty() {
        writeln!(out, "\n{}", s.comments)?;
    }
    writeln!(out)?;
    if s.methods.is_empty() {
        return writeln!(out, "no methods");
    }
    let w = s.methods.iter().map(|m| m.name.len()).max().unwrap_or(0);
    for m in s.methods {
        let mut line = format!("  {:w$}  {}", m.name, m.summary());
        if let Some(tag) = streaming_tag(m.streaming) {
            line.push_str(" [");
            line.push_str(tag);
            line.push(']');
        }
        writeln!(out, "{}", line.trim_end())?;
    }
    writeln!(out, "\nShow a method's docs: {prog} {} <METHOD>", s.alias)
}

pub fn describe(s: &Service, m: &Method, prog: &str, out: &mut impl Write) -> io::Result<()> {
    writeln!(out, "{}/{}", s.full_name, m.name)?;
    writeln!(out, "  request:     {}", m.input_type)?;
    writeln!(out, "  response:    {}", m.output_type)?;
    writeln!(out, "  idempotency: {}", idempotency_label(m.idempotency))?;
    writeln!(out, "  streaming:   {}", m.streaming.as_str())?;
    if !m.comments.is_empty() {
        writeln!(out, "\n{}", m.comments)?;
    }
    if m.streaming.is_unary() {
        writeln!(out, "\nCall it: {prog} {} {} '<JSON>'", s.alias, m.name)
    } else {
        writeln!(out, "\nStreaming methods can't be called from here.")
    }
}

/// Pretty-prints a JSON response; anything else is written as is.
pub fn response(body: &[u8], out: &mut impl Write) -> io::Result<()> {
    if let Ok(v) = serde_json::from_slice::<serde_json::Value>(body) {
        serde_json::to_writer_pretty(&mut *out, &v)?;
        return writeln!(out);
    }
    out.write_all(body)?;
    if !body.ends_with(b"\n") {
        writeln!(out)?;
    }
    Ok(())
}

const fn streaming_tag(s: Streaming) -> Option<&'static str> {
    match s {
        Streaming::Unary => None,
        Streaming::Client => Some("client streaming"),
        Streaming::Server => Some("server streaming"),
        Streaming::Bidi => Some("bidi streaming"),
    }
}

const fn idempotency_label(i: Idempotency) -> &'static str {
    match i {
        Idempotency::Unknown => "unknown",
        Idempotency::NoSideEffects => "no side effects",
        Idempotency::Idempotent => "idempotent",
    }
}
