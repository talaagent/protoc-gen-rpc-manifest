# rpc-manifest-cli

A drop-in `rpc` subcommand for admin CLIs, driven by a manifest from
`protoc-gen-rpc-manifest`:

```text
myadmin rpc                              # list services
myadmin rpc user                         # list UserService's methods
myadmin rpc user get-user                # show GetUser's docs
myadmin rpc user GetUser '{"id":"7"}'    # call it, pretty-print the reply
myadmin rpc user GetUser @req.json       # body from a file
echo '{}' | myadmin rpc user ListUsers - # body from stdin
```

Names are matched case-insensitively (with `-`/`_` ignored) and a typo
gets did-you-mean suggestions. Calls go over Connect's unary JSON protocol
with [`rpc_manifest::connect::Client`].

Features: `bpaf` (default) provides `rpc_args()` and `rpc_command()`;
`clap` derives `clap::Args` on [`RpcArgs`]; `tls` enables `https://`;
`miette` derives `Diagnostic` on [`Error`].

See the [workspace README](https://github.com/talaagent/protoc-gen-rpc-manifest#embedding-rpc-in-an-admin-cli)
for a complete embedding example.
