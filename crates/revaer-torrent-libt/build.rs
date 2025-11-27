#![allow(clippy::all)]

use std::env;
use std::path::PathBuf;

const MIN_VERSION: &str = "2.0.10";

fn main() {
    println!("cargo:rerun-if-env-changed=LIBTORRENT_INCLUDE_DIR");
    println!("cargo:rerun-if-env-changed=LIBTORRENT_LIB_DIR");
    println!("cargo:rerun-if-env-changed=LIBTORRENT_BUNDLE_DIR");

    let mut bridge = cxx_build::bridge("src/ffi/bridge.rs");
    bridge.flag_if_supported("-std=c++17");
    bridge.file("src/ffi/session.cpp");

    let include_dir = PathBuf::from("src/ffi/include");
    bridge.include(&include_dir);

    if let Some((include, lib)) = bundled_paths() {
        bridge.include(&include);
        println!("cargo:rustc-link-search=native={}", lib.display());
        emit_link_libs(vec!["torrent-rasterbar".to_string()]);
        bridge.compile("revaer-libtorrent");
        emit_reruns();
        return;
    }

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

    if let Some(path) = env::var_os("LIBTORRENT_INCLUDE_DIR") {
        bridge.include(PathBuf::from(path));
    }

    let mut libs: Vec<String> = Vec::new();
    if let Some(path) = env::var_os("LIBTORRENT_LIB_DIR") {
        println!(
            "cargo:rustc-link-search=native={}",
            PathBuf::from(&path).display()
        );
        libs.push("torrent-rasterbar".to_string());
    } else if let Ok(libtorrent) = pkg_config::Config::new()
        .atleast_version(MIN_VERSION)
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
    emit_link_libs(libs);
    emit_reruns();
}

fn bundled_paths() -> Option<(PathBuf, PathBuf)> {
    let root = env::var_os("LIBTORRENT_BUNDLE_DIR").map(PathBuf::from)?;
    let include = root.join("include");
    let lib = root.join("lib");
    if include.join("libtorrent").exists() && lib.exists() {
        Some((include, lib))
    } else {
        None
    }
}

fn emit_link_libs(libs: Vec<String>) {
    for lib in libs {
        println!("cargo:rustc-link-lib={lib}");
    }
}

fn emit_reruns() {
    // Re-run if the bridge or C++ sources change.
    println!("cargo:rerun-if-changed=src/ffi/bridge.rs");
    println!("cargo:rerun-if-changed=src/ffi/include/revaer/session.hpp");
    println!("cargo:rerun-if-changed=src/ffi/session.cpp");
}
