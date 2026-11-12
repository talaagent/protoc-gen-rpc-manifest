//! The `rpc` arguments, for bpaf and/or clap.

/// Arguments of the `rpc` subcommand
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "bpaf", derive(bpaf::Bpaf))]
#[cfg_attr(feature = "bpaf", bpaf(generate(rpc_args)))]
#[cfg_attr(feature = "clap", derive(clap::Args))]
pub struct RpcArgs {
    /// Service alias or full name; omit to list services
    #[cfg_attr(feature = "bpaf", bpaf(positional("SERVICE")))]
    #[cfg_attr(feature = "clap", arg(value_name = "SERVICE"))]
    pub service: Option<String>,

    /// Method name (case-insensitive); omit to list methods
    #[cfg_attr(feature = "bpaf", bpaf(positional("METHOD")))]
    #[cfg_attr(feature = "clap", arg(value_name = "METHOD"))]
    pub method: Option<String>,

    /// JSON request, @file, or - for stdin; omit to show the method's docs
    #[cfg_attr(feature = "bpaf", bpaf(positional("JSON")))]
    #[cfg_attr(feature = "clap", arg(value_name = "JSON"))]
    pub body: Option<String>,
}

/// The `rpc` subcommand as a bpaf command parser, ready to put in a
/// `construct!([...])` of subcommands.
#[cfg(feature = "bpaf")]
pub fn rpc_command() -> impl bpaf::Parser<RpcArgs> {
    use bpaf::Parser as _;
    rpc_args()
        .to_options()
        .descr("Call any RPC by service and method name")
        .footer(
            "With no arguments, lists services; with a service, lists its methods; \
             with a method, shows its docs; with a JSON body, calls it.",
        )
        .command("rpc")
}
