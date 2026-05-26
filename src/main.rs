use std::process;

mod channel;
mod codegen;
mod connect;
mod proxy;

/// Print usage information for the metropipe CLI.
fn print_usage() {
    eprintln!("metropipe — share data between processes on the same machine");
    eprintln!();
    eprintln!("Usage:");
    eprintln!("  metropipe connect <service>          Interactive REPL (default)");
    eprintln!("  metropipe connect <service> --send   Send one request, print response, exit");
    eprintln!("  metropipe connect <service> --listen Act as provider (receive requests)");
    eprintln!("  metropipe connect <service> --gen-stubs  Generate client stubs for 9 languages");
    eprintln!("  metropipe bind <library>             Generate .dbv + stubs from a library");
    eprintln!("  metropipe proxy <service>            stdin/stdout bridge for any language");
    eprintln!("  metropipe --help                     Show this help");
}

/// Entry point for the metropipe binary. Dispatches to subcommands.
fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        print_usage();
        process::exit(1);
    }

    let result = match args[1].as_str() {
        "connect" => {
            if args.len() < 3 {
                eprintln!("Error: Missing service name");
                eprintln!("Usage: metropipe connect <service> [--send <data>] [--listen] [--gen-stubs]");
                process::exit(1);
            }
            connect::run_connect(&args[2..])
        }
        "bind" => {
            if args.len() < 3 {
                eprintln!("Error: Missing library path");
                eprintln!("Usage: metropipe bind <library> [--out <dir>]");
                process::exit(1);
            }
            // Simplified: generate .dbv and stubs for the given library name
            let lib_name = &args[2];
            let out_dir = if args.len() > 4 && args[3] == "--out" {
                args[4].clone()
            } else {
                format!("lib/ffi/generated/{}", lib_name)
            };
            codegen::generate_all_stubs(lib_name, &out_dir)
        }
        "proxy" => {
            if args.len() < 3 {
                eprintln!("Error: Missing service name");
                eprintln!("Usage: metropipe proxy <service>");
                process::exit(1);
            }
            proxy::run_proxy(&args[2])
        }
        "-h" | "--help" | "help" => {
            print_usage();
            Ok(())
        }
        _ => {
            eprintln!("Error: Unknown command '{}'", args[1]);
            print_usage();
            process::exit(1);
        }
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}
