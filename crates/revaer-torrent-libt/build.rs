#![allow(clippy::all)]

use std::env;
use std::path::PathBuf;

fn main() {
    let mut bridge = cxx_build::bridge("src/ffi/bridge.rs");
    bridge.flag_if_supported("-std=c++17");
    bridge.file("src/ffi/session.cpp");

    let include_dir = PathBuf::from("src/ffi/include");
    bridge.include(&include_dir);

    for prefix in ["/opt/homebrew", "/usr/local"] {
        let root = PathBuf::from(prefix);
        let include = root.join("include");
        if include.join("libtorrent").exists() {
            bridge.include(&include);
        }
        let lib = root.join("lib");
        if lib.join("libtorrent-rasterbar.dylib").exists()
            || lib.join("libtorrent-rasterbar.a").exists()
        {
            println!("cargo:rustc-link-search=native={}", lib.display());
        }
    }

    // Allow callers to provide explicit include/lib directories.
    if let Some(path) = env::var_os("LIBTORRENT_INCLUDE_DIR") {
        bridge.include(PathBuf::from(path));
    }

    let mut libs: Vec<String> = Vec::new();
    if let Some(path) = env::var_os("LIBTORRENT_LIB_DIR") {
        println!(
            "cargo:rustc-link-search=native={}",
            PathBuf::from(&path).display()
        );
    } else if let Ok(libtorrent) = pkg_config::Config::new()
        .atleast_version("2.0.0")
        .probe("libtorrent-rasterbar")
    {
        let include_paths = libtorrent.include_paths;
        for path in include_paths {
            bridge.include(path);
        }
        let link_paths = libtorrent.link_paths;
        for lib_path in link_paths {
            println!("cargo:rustc-link-search=native={}", lib_path.display());
        }
        let pkg_libs = libtorrent.libs;
        for lib in &pkg_libs {
            println!("cargo:rustc-link-lib={lib}");
        }
        libs.extend(pkg_libs);
    } else {
        // Fallback to default library name if pkg-config lookup failed.
        libs.push("torrent-rasterbar".to_string());
    }

    bridge.compile("revaer-libtorrent");

    for lib in libs {
        println!("cargo:rustc-link-lib={lib}");
    }

    // Re-run if the bridge or C++ sources change.
    println!("cargo:rerun-if-changed=src/ffi/bridge.rs");
    println!("cargo:rerun-if-changed=src/ffi/include/revaer/session.hpp");
    println!("cargo:rerun-if-changed=src/ffi/session.cpp");
}
