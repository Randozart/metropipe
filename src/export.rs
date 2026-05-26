use std::fs;
use std::path::{Path, PathBuf};
use std::collections::HashMap;

use crate::codegen;

/// A parsed function signature extracted from a source file.
#[derive(Debug, Clone)]
pub struct FunctionSig {
    pub name: String,
    pub params: Vec<(String, String)>,
    pub return_types: Vec<(String, String)>,
    pub source_lang: String,
}

/// Output mode for generated files.
#[derive(Debug, Clone, PartialEq)]
pub enum OutputMode {
    Namespace,
    Flat,
    Unify,
}

/// Run the metropipe export command.
///
/// Parses a function from a source file and generates typed stubs
/// + provider scripts for the targeted languages.
pub fn run_export(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let func_name = &args[0];
    let source_path = Path::new(&args[1]);

    // Parse flags
    let mut targets: Vec<String> = Vec::new();
    let mut out_dir = PathBuf::from("metropipe");
    let mut mode = OutputMode::Namespace;
    let mut i = 2;

    while i < args.len() {
        match args[i].as_str() {
            "--target" if i + 1 < args.len() => {
                i += 1;
                while i < args.len() && !args[i].starts_with("--") {
                    targets.push(args[i].clone());
                    i += 1;
                }
                continue;
            }
            "--out" if i + 1 < args.len() => {
                out_dir = PathBuf::from(&args[i + 1]);
                i += 2;
                continue;
            }
            "--namespace" => { mode = OutputMode::Namespace; i += 1; }
            "--flat" => { mode = OutputMode::Flat; i += 1; }
            "--unify" => { mode = OutputMode::Unify; i += 1; }
            _ => i += 1,
        }
    }

    // Default targets: all languages
    if targets.is_empty() {
        targets = vec![
            "c".into(), "python".into(), "js".into(), "rust".into(),
            "go".into(), "java".into(), "csharp".into(), "ruby".into(), "bash".into(),
        ];
    }

    // Detect source language from extension
    let ext = source_path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let source_lang = match ext.as_str() {
        "py" => "python",
        "rs" => "rust",
        "go" => "go",
        "c" | "h" => "c",
        "js" | "mjs" => "js",
        "ts" => "ts",
        "rb" => "ruby",
        "java" => "java",
        "cs" => "csharp",
        _ => "unknown",
    };

    // Parse the function signature from the source file
    let source_code = fs::read_to_string(source_path)?;
    let sig = parse_function(source_lang, func_name, &source_code);

    // Build output paths
    ensure_dir(&out_dir)?;

    match mode {
        OutputMode::Namespace => {
            let func_dir = out_dir.join(func_name);
            ensure_dir(&func_dir)?;
            for lang in &targets {
                write_stub_for_lang(lang, &sig, &func_name, &func_dir, mode.clone())?;
            }
            let provider = generate_provider(&sig, &source_path);
            fs::write(func_dir.join("provider.py"), &provider)?;
            println!("Exported {} to {}", func_name, func_dir.display());
        }
        OutputMode::Flat => {
            for lang in &targets {
                write_stub_for_lang(lang, &sig, &func_name, &out_dir, mode.clone())?;
            }
            let provider = generate_provider(&sig, &source_path);
            fs::write(out_dir.join("provider.py"), &provider)?;
            println!("Exported {} to {}", func_name, out_dir.display());
        }
        OutputMode::Unify => {
            for lang in &targets {
                let (_filename, content) = generate_target_stub(lang, &sig, &func_name)?;
                let path = out_dir.join(format!("metropipe.{}", ext_for_target(lang)));
                let mut existing = String::new();
                if path.exists() {
                    existing = fs::read_to_string(&path)?;
                }
                fs::write(&path, format!("{}\n{}", existing, content))?;
            }
            let provider = generate_provider(&sig, &source_path);
            let prov_path = out_dir.join("metropipe_provider.py");
            let mut existing = String::new();
            if prov_path.exists() { existing = fs::read_to_string(&prov_path)?; }
            fs::write(&prov_path, format!("{}\n{}", existing, provider))?;
            println!("Unified export to {}", out_dir.display());
        }
    }

    Ok(())
}

fn ext_for_target(lang: &str) -> &str {
    match lang {
        "c" | "c-direct" => "h",
        "c-linker" => "h",
        "rust" => "rs",
        "go" => "go",
        "java" => "java",
        "csharp" => "cs",
        "ruby" => "rb",
        "js" => "js",
        "python" => "py",
        "bash" => "sh",
        _ => "txt",
    }
}

fn ensure_dir(dir: &Path) -> Result<(), String> {
    if !dir.exists() {
        fs::create_dir_all(dir).map_err(|e| format!("mkdir {}: {}", dir.display(), e))?;
    }
    Ok(())
}

/// Parse a function signature from source code given the language and function name.
fn parse_function(lang: &str, func_name: &str, source: &str) -> FunctionSig {
    match lang {
        "python" => parse_python_function(func_name, source),
        "rust" => parse_rust_function(func_name, source),
        "go" => parse_go_function(func_name, source),
        "c" => parse_c_function(func_name, source),
        _ => FunctionSig {
            name: func_name.to_string(),
            params: vec![],
            return_types: vec![("Result".into(), "Data".into())],
            source_lang: lang.to_string(),
        },
    }
}

fn parse_python_function(func_name: &str, source: &str) -> FunctionSig {
    let mut params: Vec<(String, String)> = Vec::new();
    let mut return_types: Vec<(String, String)> = vec![("return".into(), "Data".into())];

    // Find `def func_name(...)` 
    let pattern = format!("def {}(", func_name);
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(&pattern) {
            let paren_end = trimmed.find(')').unwrap_or(trimmed.len());
            let params_str = &trimmed[pattern.len()..paren_end];
            for p in params_str.split(',') {
                let p = p.trim();
                if p.is_empty() || p == "self" { continue; }
                if let Some(idx) = p.find(':') {
                    let name = p[..idx].trim();
                    let ty = p[idx+1..].trim();
                    params.push((name.to_string(), ty.to_string()));
                } else {
                    params.push((p.to_string(), "Data".into()));
                }
            }

            // Check return type annotation
            let rest = &trimmed[paren_end+1..];
            if let Some(arrow) = rest.find("->") {
                let ret_str = rest[arrow+2..].trim().trim_end_matches(':');
                if ret_str.contains("Tuple") || ret_str.contains(',') {
                    return_types = vec![("result".into(), ret_str.trim().into())];
                } else {
                    return_types = vec![("return".into(), ret_str.trim().into())];
                }
            }
            break;
        }
    }

    FunctionSig {
        name: func_name.to_string(),
        params,
        return_types,
        source_lang: "python".into(),
    }
}

fn parse_rust_function(func_name: &str, source: &str) -> FunctionSig {
    let mut params: Vec<(String, String)> = Vec::new();
    let mut return_types: Vec<(String, String)> = vec![("result".into(), "Data".into())];

    let pattern = format!("fn {}(", func_name);
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(&pattern) {
            let paren_end = trimmed.find(')').unwrap_or(trimmed.len());
            let params_str = &trimmed[pattern.len()..paren_end];
            for p in params_str.split(',') {
                let p = p.trim();
                if p.is_empty() { continue; }
                if let Some(idx) = p.find(':') {
                    let name = p[..idx].trim();
                    let ty = p[idx+1..].trim().trim_end_matches(',');
                    params.push((name.to_string(), ty.to_string()));
                }
            }

            let rest = &trimmed[paren_end+1..];
            if let Some(arrow) = rest.find("->") {
                let ret_str = rest[arrow+2..].trim().trim_end_matches('{').trim();
                return_types = vec![("result".into(), ret_str.into())];
            }
            break;
        }
    }

    FunctionSig {
        name: func_name.to_string(),
        params,
        return_types,
        source_lang: "rust".into(),
    }
}

fn parse_go_function(func_name: &str, source: &str) -> FunctionSig {
    let mut params: Vec<(String, String)> = Vec::new();
    let mut return_types: Vec<(String, String)> = vec![("result".into(), "Data".into())];

    let pattern = format!("func {}(", func_name);
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(&pattern) {
            let paren_end = trimmed.find(')').unwrap_or(trimmed.len());
            let params_str = &trimmed[pattern.len()..paren_end];
            for p in params_str.split(',') {
                let p = p.trim();
                if p.is_empty() { continue; }
                // Go: name type (name and type separated by space)
                let parts: Vec<&str> = p.split_whitespace().collect();
                if parts.len() >= 2 {
                    let name = parts[0];
                    let ty = parts[1..].join(" ");
                    params.push((name.to_string(), ty));
                } else {
                    params.push((p.to_string(), "Data".into()));
                }
            }

            let rest = &trimmed[paren_end+1..];
            if let Some(paren) = rest.find('(') {
                let ret_str = rest[1..paren].trim();
                if !ret_str.is_empty() {
                    for r in ret_str.split(',') {
                        let r = r.trim();
                        if !r.is_empty() {
                            return_types = vec![("result".into(), r.into())];
                        }
                    }
                }
            } else if let Some(space) = rest.find(' ') {
                let ret_str = rest[..space].trim();
                if !ret_str.is_empty() && ret_str != "{" {
                    return_types = vec![("result".into(), ret_str.into())];
                }
            }
            break;
        }
    }

    FunctionSig {
        name: func_name.to_string(),
        params,
        return_types,
        source_lang: "go".into(),
    }
}

fn parse_c_function(func_name: &str, source: &str) -> FunctionSig {
    let mut params: Vec<(String, String)> = Vec::new();
    let mut return_type = "void".to_string();

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.contains(&format!("{})", func_name)) || trimmed.contains(&format!("{},", func_name)) {
            let before = trimmed.split(&format!("{}(", func_name)).next().unwrap_or("").trim();
            if !before.is_empty() {
                return_type = before.to_string();
            }
            if let Some(start) = trimmed.find(&format!("{}(", func_name)) {
                let from_paren = &trimmed[start..];
                let paren_end = from_paren.find(')').unwrap_or(from_paren.len());
                let params_str = &from_paren[func_name.len()+1..paren_end];
                for p in params_str.split(',') {
                    let p = p.trim();
                    if p.is_empty() || p == "void" { continue; }
                    let parts: Vec<&str> = p.split_whitespace().collect();
                    if parts.len() >= 2 {
                        let ty = parts[..parts.len()-1].join(" ");
                        let name = parts[parts.len()-1];
                        params.push((name.to_string(), ty));
                    }
                }
            }
            break;
        }
    }

    FunctionSig {
        name: func_name.to_string(),
        params,
        return_types: vec![("return".into(), return_type)],
        source_lang: "c".into(),
    }
}

/// Write stubs for a language, handling multi-file outputs (c-linker).
fn write_stub_for_lang(lang: &str, sig: &FunctionSig, func_name: &str, dir: &Path, _mode: OutputMode) -> Result<(), String> {
    if lang == "c-linker" {
        // Write .h
        let (_, h_content) = generate_target_stub("c-linker", sig, func_name)?;
        fs::write(dir.join(format!("{}.h", func_name)), &h_content).map_err(|e| e.to_string())?;
        // Write .c
        let c_content = format!(r#"#include "{func_name}.h"
/* Compiled C stub for {func_name} — compiles to a .so for direct linking.
 * Makefile generates: lib{func_name}.so */
"#, func_name = func_name);
        fs::write(dir.join(format!("{}.c", func_name)), &c_content).map_err(|e| e.to_string())?;
        // Write Makefile
        let makefile = format!(
"all:\n\tcc -shared -o lib{0}.so {0}.c -lrt\nclean:\n\trm -f lib{0}.so\n",
            func_name);
        fs::write(dir.join("Makefile"), &makefile).map_err(|e| e.to_string())?;
        return Ok(());
    }
    let (filename, content) = generate_target_stub(lang, sig, func_name)?;
    fs::write(dir.join(&filename), &content).map_err(|e| e.to_string())
}

/// Generate a typed stub in the target language for a function signature.
fn generate_target_stub(lang: &str, sig: &FunctionSig, func_name: &str) -> Result<(String, String), String> {
    let (layout, req_size) = compute_layout(&sig.params);
    let resp_size = type_size_bytes(sig.return_types.first().map(|(_,t)| t.as_str()).unwrap_or("Data"));

    match lang {
        "rust" => {
            let params_str: Vec<String> = sig.params.iter()
                .map(|(n, t)| format!("{}: {}", n, type_to_rust(t)))
                .collect();
            let ret_str = if sig.return_types.len() == 1 {
                type_to_rust(&sig.return_types[0].1)
            } else {
                "Vec<u8>".into()
            };

            // Generate zero-copy serialization for each parameter
            let mut write_fields = String::new();
            let mut read_fields = String::new();
            let mut pack_fields = String::new();
            for ((offset, size), (name, ty)) in layout.iter().zip(&sig.params) {
                match ty.trim() {
                    "str" | "String" | "string" => {
                        write_fields.push_str(&format!(
                            "    let bytes_{} = {}.as_bytes();\n    buf[32+{}..32+{}+bytes_{}.len().min({})].copy_from_slice(&bytes_{}[..bytes_{}.len().min({})]);\n",
                            name, name, offset, offset, name, size, name, name, size
                        ));
                        read_fields.push_str(&format!(
                            "    let {} = String::from_utf8_lossy(&buf[32+{}..32+{}+{}]).trim_end_matches('\\x00').to_string();\n",
                            name, offset, offset, size
                        ));
                    }
                    "bool" | "Bool" => {
                        write_fields.push_str(&format!(
                            "    buf[32+{}] = if {} {{ 1 }} else {{ 0 }};\n", offset, name
                        ));
                        read_fields.push_str(&format!(
                            "    let {} = buf[32+{}] != 0;\n", name, offset
                        ));
                    }
                    _ => {
                        let sz = type_size_bytes(ty);
                        write_fields.push_str(&format!(
                            "    buf[32+{}..32+{}+{}].copy_from_slice(&{}.to_le_bytes());\n", offset, offset, sz, name
                        ));
                        read_fields.push_str(&format!(
                            "    let {} = {}::from_le_bytes(buf[32+{}..32+{}+{}].try_into().unwrap());\n", name, type_to_rust(ty), offset, offset, sz
                        ));
                    }
                }
                pack_fields.push_str(&format!("buf[32+{}..].as_ptr() as usize, ", offset));
            }

            let content = format!(r#"
use std::fs::OpenOptions;
use std::os::unix::io::AsRawFd;

const STATUS_IDLE: u32 = 0;
const STATUS_CONSUMER_REQ: u32 = 1;
const STATUS_PROVIDER_RES: u32 = 3;
const REQ_SIZE: usize = {req_size};
const RESP_SIZE: usize = {resp_size};

pub fn {func_name}({params}) -> Result<{ret}, String> {{
    let shm = std::env::var("METROPIPE_DIR").unwrap_or_else(|_| "/dev/shm".into());
    let path = format!("{{}}/metro_{func_name}");
    let fd = OpenOptions::new().read(true).write(true).open(&path).map_err(|e| format!("open: {{}}", e))?;
    let len = std::fs::metadata(&path).map_err(|e| e.to_string())?.len() as usize;
    let ptr = unsafe {{ libc::mmap(std::ptr::null_mut(), len, libc::PROT_READ|libc::PROT_WRITE, libc::MAP_SHARED, fd.as_raw_fd(), 0) }};
    if ptr == libc::MAP_FAILED {{ return Err("mmap".into()); }}
    let buf = unsafe {{ std::slice::from_raw_parts_mut(ptr as *mut u8, len) }};

    let start = std::time::Instant::now();
    while u32::from_le_bytes(buf[0..4].try_into().unwrap()) != STATUS_IDLE {{
        if start.elapsed().as_millis() > 5000 {{ return Err("timeout".into()); }}
    }}
{write_fields}
    buf[8..12].copy_from_slice(&(REQ_SIZE as u32).to_le_bytes());
    buf[0..4].copy_from_slice(&STATUS_CONSUMER_REQ.to_le_bytes());

    let rs = std::time::Instant::now();
    loop {{
        let s = u32::from_le_bytes(buf[0..4].try_into().unwrap());
        if s == STATUS_PROVIDER_RES {{
{read_fields}
            let result: {ret} = {read_result};
            buf[0..4].copy_from_slice(&STATUS_IDLE.to_le_bytes());
            let _ = unsafe {{ libc::munmap(ptr as *mut u8, len) }};
            return Ok(result);
        }}
        if rs.elapsed().as_millis() > 5000 {{ return Err("timeout".into()); }}
    }}
}}
"#, func_name = func_name, params = params_str.join(", "), ret = ret_str,
                req_size = req_size, resp_size = resp_size,
                write_fields = write_fields, read_fields = read_fields,
                read_result = {
                    if sig.return_types.len() == 1 {
                        format!("{}", sig.return_types[0].1)
                    } else {
                        format!("vec![]")
                    }
                });
            Ok(("stub.rs".into(), content))
        }
        "python" => {
            let mut write_fields = String::new();
            let mut read_fields = String::new();
            for ((offset, size), (name, ty)) in layout.iter().zip(&sig.params) {
                match ty.trim() {
                    "str" | "String" | "string" => {
                        write_fields.push_str(&format!(
                            "    data = {}.encode()[:{}]\n    mm[32+{}:32+{}+len(data)] = data\n", name, size, offset, offset
                        ));
                    }
                    "int" | "Int" | "i32" | "float" | "Float" | "f32" => {
                        write_fields.push_str(&format!(
                            "    struct.pack_into(\"<i\", mm, 32+{}, int({}))\n", offset, name
                        ));
                        read_fields.push_str(&format!(
                            "    {} = struct.unpack_from(\"<i\", mm, 32+{})[0]\n", name, offset
                        ));
                    }
                    "bool" | "Bool" => {
                        write_fields.push_str(&format!(
                            "    struct.pack_into(\"<?\", mm, 32+{}, {})\n", offset, name
                        ));
                        read_fields.push_str(&format!(
                            "    {} = bool(struct.unpack_from(\"<?\", mm, 32+{})[0])\n", name, offset
                        ));
                    }
                    _ => {
                        write_fields.push_str(&format!(
                            "    struct.pack_into(\"<i\", mm, 32+{}, int({}))\n", offset, name
                        ));
                        read_fields.push_str(&format!(
                            "    {} = struct.unpack_from(\"<i\", mm, 32+{})[0]\n", name, offset
                        ));
                    }
                }
            }

            let content = format!(r#"
import struct, time, os
SHM_DIR = os.environ.get("METROPIPE_DIR", "/dev/shm")
SHM_PATH = os.path.join(SHM_DIR, "metro_{func_name}")

STATUS_IDLE = 0
STATUS_CONSUMER_REQ = 1
STATUS_PROVIDER_RES = 3

def {func_name}({params}):
    fd = open(SHM_PATH, "r+b") if os.path.exists(SHM_PATH) else open(SHM_PATH, "w+b")
    mm = __import__("mmap").mmap(fd.fileno(), 0)
    while struct.unpack_from("<I", mm, 0)[0] != STATUS_IDLE:
        time.sleep(0.001)
{write_fields}
    struct.pack_into("<I", mm, 8, {req_size})
    struct.pack_into("<I", mm, 0, STATUS_CONSUMER_REQ)
    while True:
        s = struct.unpack_from("<I", mm, 0)[0]
        if s == STATUS_PROVIDER_RES:
            resp = bytes(mm[32:32+{resp_size}])
            struct.pack_into("<I", mm, 0, STATUS_IDLE)
            mm.close()
            return resp
        time.sleep(0.001)
"#, func_name = func_name, params = sig.params.iter().map(|(n,t)| format!("{}: {}", n, type_to_python(t))).collect::<Vec<_>>().join(", "),
                req_size = req_size, resp_size = resp_size, write_fields = write_fields);
            Ok(("stub.py".into(), content))
        }
        "c-direct" => {
            let params_decl: Vec<String> = sig.params.iter()
                .map(|(n, t)| format!("{} {}", type_to_c(t), n))
                .collect();
            let ret_c = if sig.return_types.len() == 1 { type_to_c(&sig.return_types[0].1) } else { "void*".into() };
            let content = format!(r#"
/* Metropipe direct function pointer registry for {func_name}
 * Zero-copy, in-process, no compilation needed.
 * Provider calls metropipe_register(), consumer calls metropipe_get_registry(). */

typedef {ret_c} (*{func_name}_fn)({c_decl});

typedef struct {{
    {func_name}_fn {func_name};
}} MetropipeRegistry;

static MetropipeRegistry registry = {{0}};

void metropipe_register(MetropipeRegistry *reg) {{
    if (reg->{func_name}) registry.{func_name} = reg->{func_name};
}}

MetropipeRegistry* metropipe_get_registry(void) {{
    return &registry;
}}
"#, func_name = func_name, ret_c = ret_c, c_decl = params_decl.join(", "));
            Ok(("registry.h".into(), content))
        }
        "c-linker" => {
            let params_decl: Vec<String> = sig.params.iter()
                .map(|(n, t)| format!("{} {}", type_to_c(t), n))
                .collect();
            let ret_c = if sig.return_types.len() == 1 { type_to_c(&sig.return_types[0].1) } else { "void*".into() };
            let header = format!(r#"
/* Compiled C stub for {func_name}
 * Compile: cc -shared -o lib{func_name}.so {func_name}.c */

#ifndef METROPIPE_{upper}_H
#define METROPIPE_{upper}_H

{ret_c} {func_name}({c_decl});

#endif
"#, upper = func_name.to_uppercase(), func_name = func_name, ret_c = ret_c, c_decl = params_decl.join(", "));
            let source = format!(r#"
#include "{func_name}.h"
#include <stdint.h>
#include <fcntl.h>
#include <unistd.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <string.h>

/* Talks to metropipe shared memory channel. */

#define STATUS_IDLE 0
#define STATUS_CONSUMER_REQ 1
#define STATUS_PROVIDER_RES 3

{ret_c} {func_name}({c_decl}) {{
    char path[256];
    const char *d = getenv("METROPIPE_DIR");
    if (d) snprintf(path, sizeof(path), "%s/metro_{func_name}", d);
    else snprintf(path, sizeof(path), "/dev/shm/metro_{func_name}");
    int fd = open(path, O_RDWR);
    if (fd < 0) return 0;
    struct stat st; fstat(fd, &st);
    void *buf = mmap(NULL, st.st_size, PROT_READ|PROT_WRITE, MAP_SHARED, fd, 0);
    if (buf == MAP_FAILED) return 0;

    // Write params at known offsets (zero-copy layout)
    // (see shared memory stubs for full implementation)
    munmap(buf, st.st_size); close(fd);
    return 0;
}}
"#, func_name = func_name, ret_c = ret_c, c_decl = params_decl.join(", "));
            let makefile = format!(
"all:\n\tcc -shared -o lib{0}.so {0}.c -lrt\nclean:\n\trm -f lib{0}.so\n",
                func_name);
            // Write both files and Makefile
            Ok(("{func_name}.h".into(), header))  // Will split into multiple files in caller
        }
        "c" => {
            // Shared memory stub: zero-copy layout
            let params_decl: Vec<String> = sig.params.iter()
                .map(|(n, t)| format!("{} {}", type_to_c(t), n))
                .collect();
            let ret_c = if sig.return_types.len() == 1 { type_to_c(&sig.return_types[0].1) } else { "void*".into() };
            let mut write_fields = String::new();
            for ((offset, size), (name, ty)) in layout.iter().zip(&sig.params) {
                match ty.trim() {
                    "str" | "String" | "string" | "const char*" => {
                        write_fields.push_str(&format!(
                            "    memcpy(buf+32+{}, {}, strnlen({}, {}));\n", offset, name, name, size
                        ));
                    }
                    "int" | "int32_t" | "i32" => {
                        write_fields.push_str(&format!(
                            "    *(int32_t*)(buf+32+{}) = {};\n", offset, name
                        ));
                    }
                    "float" | "f32" => {
                        write_fields.push_str(&format!(
                            "    *(float*)(buf+32+{}) = {};\n", offset, name
                        ));
                    }
                    "double" | "f64" | "int64_t" => {
                        write_fields.push_str(&format!(
                            "    *(int64_t*)(buf+32+{}) = {};\n", offset, name
                        ));
                    }
                    _ => {
                        write_fields.push_str(&format!(
                            "    *(int32_t*)(buf+32+{}) = {};\n", offset, name
                        ));
                    }
                }
            }
            let content = format!(r#"
#include <stdint.h>
#include <stdatomic.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <fcntl.h>
#include <sys/mman.h>
#include <sys/stat.h>

#define STATUS_IDLE 0
#define STATUS_CONSUMER_REQ 1
#define STATUS_PROVIDER_RES 3

{ret_c} {func_name}({c_decl}) {{
    char path[256];
    const char *d = getenv("METROPIPE_DIR");
    if (d) snprintf(path, sizeof(path), "%s/metro_{func_name}", d);
    else snprintf(path, sizeof(path), "/dev/shm/metro_{func_name}");
    int fd = open(path, O_RDWR);
    if (fd < 0) return 0;
    struct stat st; fstat(fd, &st);
    void *buf = mmap(NULL, st.st_size, PROT_READ|PROT_WRITE, MAP_SHARED, fd, 0);
    if (buf == MAP_FAILED) return 0;

    volatile uint32_t *status = (volatile uint32_t*)buf;
    volatile uint32_t *size = (volatile uint32_t*)(buf + 8);
    while (*status != STATUS_IDLE);
{write_fields}
    *size = {req_size};
    *status = STATUS_CONSUMER_REQ;
    while (*status != STATUS_PROVIDER_RES);

    {ret_c} result = 0;
{read_result}
    *status = STATUS_IDLE;
    munmap(buf, st.st_size); close(fd);
    return result;
}}
"#, func_name = func_name, ret_c = ret_c, c_decl = params_decl.join(", "),
                req_size = req_size, write_fields = write_fields,
                read_result = if ret_c != "void" { format!("    result = *({ret_c}*)(buf + 32);", ret_c = ret_c) } else { String::new() });
            Ok(("stub.h".into(), content))
        }
        _ => {
            let content = format!("// metropipe stub for {func_name} ({lang})\n");
            Ok((format!("stub.{}", ext_for_target(lang)), content))
        }
    }
}

/// Generate a provider script that wraps the real function in a metropipe poll loop.
fn generate_provider(sig: &FunctionSig, source_path: &Path) -> String {
    let source_stem = source_path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("module");

    match sig.source_lang.as_str() {
        "python" => format!(r#"
import struct, time, os, json, importlib

mod = importlib.import_module("{source_stem}")
{func_name} = getattr(mod, "{func_name}")

SHM_DIR = os.environ.get("METROPIPE_DIR", "/dev/shm")
SHM_PATH = os.path.join(SHM_DIR, "metro_{func_name}")
if not os.path.exists(SHM_PATH):
    os.makedirs(os.path.dirname(SHM_PATH), exist_ok=True)
    with open(SHM_PATH, "wb") as f:
        f.truncate(32 + 4096)

fd = open(SHM_PATH, "r+b")
mm = __import__("mmap").mmap(fd.fileno(), 0)

STATUS_IDLE = 0
STATUS_CONSUMER_REQ = 1
STATUS_PROVIDER_RES = 3

print("metropipe provider: {func_name} listening on", SHM_PATH)
while True:
    if struct.unpack_from("<I", mm, 0)[0] == STATUS_CONSUMER_REQ:
        sz = struct.unpack_from("<I", mm, 8)[0]
        req_data = json.loads(bytes(mm[32:32+sz]).decode())
        {args_unpack}
        result = {func_name}({args_call})
        resp = json.dumps({{"result": result}}).encode()
        mm[32:32+len(resp)] = resp
        struct.pack_into("<I", mm, 8, len(resp))
        struct.pack_into("<I", mm, 0, STATUS_PROVIDER_RES)
    time.sleep(0.001)
"#, source_stem = source_stem, func_name = sig.name,
                    args_unpack = sig.params.iter().map(|(n,_)| format!("        {} = req_data[\"{}\"]", n, n)).collect::<Vec<_>>().join("\n"),
                    args_call = sig.params.iter().map(|(n,_)| n.clone()).collect::<Vec<_>>().join(", ")),
        _ => format!("# provider for {} — run with your language's runtime\n", sig.name),
    }
}

fn type_to_rust(ty: &str) -> &str {
    match ty.trim() {
        "int" | "Int" | "i32" | "i64" => "i64",
        "float" | "Float" | "f32" | "f64" => "f64",
        "str" | "String" | "string" => "String",
        "bool" | "Bool" => "bool",
        "bytes" | "Data" => "Vec<u8>",
        _ => "String",
    }
}

fn type_to_python(ty: &str) -> &str {
    match ty.trim() {
        "int" | "Int" | "i32" | "i64" => "int",
        "float" | "Float" | "f32" | "f64" => "float",
        "str" | "String" | "string" => "str",
        "bool" | "Bool" => "bool",
        "bytes" | "Data" => "bytes",
        _ => "Any",
    }
}

fn type_to_c(ty: &str) -> &str {
    match ty.trim() {
        "int" | "Int" | "i32" => "int32_t",
        "i64" | "long" => "int64_t",
        "float" | "Float" | "f32" => "float",
        "f64" | "double" => "double",
        "str" | "String" | "string" => "const char*",
        "bool" | "Bool" => "uint8_t",
        "Data" | "bytes" => "const uint8_t*",
        _ => "void*",
    }
}

fn type_size_bytes(ty: &str) -> usize {
    match ty.trim() {
        "bool" | "Bool" | "uint8_t" | "int8_t" => 1,
        "int32_t" | "int" | "Int" | "i32" | "float" | "Float" | "f32" | "uint32_t" => 4,
        "i64" | "int64_t" | "f64" | "double" | "uint64_t" => 8,
        "str" | "String" | "string" | "const char*" => 256,
        "Data" | "bytes" | "const uint8_t*" => 4096,
        _ => 4,
    }
}

/// Compute byte offsets for each parameter in a zero-copy buffer layout.
/// Returns (offset, size) pairs for each param, and the total request size.
fn compute_layout(params: &[(String, String)]) -> (Vec<(usize, usize)>, usize) {
    let mut offset = 0usize;
    let mut layout = Vec::new();
    for (_, ty) in params {
        let size = type_size_bytes(ty);
        // Align to 4 bytes
        offset = (offset + 3) & !3;
        layout.push((offset, size));
        offset += size;
    }
    (layout, offset)
}
