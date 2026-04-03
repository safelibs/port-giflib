use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const LEGACY_SOURCES: &[&str] = &[
    "dgif_lib.c",
    "egif_lib.c",
    "gifalloc.c",
    "gif_err.c",
    "gif_font.c",
    "gif_hash.c",
    "openbsd-reallocarray.c",
    "quantize.c",
];

const LEGACY_HEADERS: &[&str] = &["gif_hash.h", "gif_lib.h", "gif_lib_private.h"];

fn main() {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let repo_root = manifest_dir.parent().unwrap();
    let original_dir = repo_root.join("original");
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let export_script = write_export_script(&original_dir, &out_dir);

    compile_legacy_core(&original_dir);

    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=m");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
        println!("cargo:rustc-link-arg-cdylib=-Wl,-soname,libgif.so.7");
        println!(
            "cargo:rustc-link-arg-cdylib=-Wl,--version-script={}",
            export_script.display()
        );
    }
}

fn compile_legacy_core(original_dir: &Path) {
    for source in LEGACY_SOURCES {
        println!(
            "cargo:rerun-if-changed={}",
            original_dir.join(source).display()
        );
    }
    for header in LEGACY_HEADERS {
        println!(
            "cargo:rerun-if-changed={}",
            original_dir.join(header).display()
        );
    }
    println!("cargo:rerun-if-changed=build.rs");

    let mut build = cc::Build::new();
    build.cargo_metadata(false);
    build.include(original_dir);
    build.pic(true);
    build.warnings(false);
    build.flag_if_supported("-std=gnu99");
    build.flag_if_supported("-fPIC");

    for source in LEGACY_SOURCES {
        build.file(original_dir.join(source));
    }

    build.compile("gif_legacy");
}

fn write_export_script(original_dir: &Path, out_dir: &Path) -> PathBuf {
    let symbols_path = original_dir.join("debian/libgif7.symbols");
    let contents = fs::read_to_string(&symbols_path).unwrap();
    let mut script = String::from("{\n  global:\n");

    println!("cargo:rerun-if-changed={}", symbols_path.display());

    for line in contents.lines().skip(2) {
        let mut fields = line.split_whitespace();
        if let Some(symbol) = fields.next() {
            let exported = symbol.strip_suffix("@Base").unwrap_or(symbol);
            script.push_str("    ");
            script.push_str(exported);
            script.push_str(";\n");
        }
    }

    script.push_str("  local:\n    *;\n};\n");

    let script_path = out_dir.join("giflib-exports.map");
    fs::write(&script_path, script).unwrap();
    script_path
}
