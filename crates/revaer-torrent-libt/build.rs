use std::env;
use std::fs;
use std::path::{Path, PathBuf};

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
        ensure_header_version(&include);
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

    let include_override = env::var_os("LIBTORRENT_INCLUDE_DIR").map(PathBuf::from);
    if let Some(path) = include_override.as_ref() {
        bridge.include(path);
    }

    let mut libs: Vec<String> = Vec::new();
    let lib_dir_override = env::var_os("LIBTORRENT_LIB_DIR").map(PathBuf::from);
    if let Some(path) = lib_dir_override.as_ref() {
        println!("cargo:rustc-link-search=native={}", path.display());
        libs.push("torrent-rasterbar".to_string());
    }

    if let Some(include_dir) = include_override.as_ref() {
        ensure_header_version(include_dir);
    } else if lib_dir_override.is_some() {
        panic!("LIBTORRENT_INCLUDE_DIR must be set to verify libtorrent version");
    }

    if libs.is_empty() {
        let libtorrent = pkg_config::Config::new()
            .atleast_version(MIN_VERSION)
            .probe("libtorrent-rasterbar")
            .unwrap_or_else(|err| {
                panic!("libtorrent-rasterbar >= {MIN_VERSION} not found via pkg-config: {err}")
            });
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

fn ensure_header_version(include_dir: &Path) {
    let header = include_dir.join("libtorrent").join("version.hpp");
    let contents = fs::read_to_string(&header).unwrap_or_else(|err| {
        panic!(
            "failed to read libtorrent version header at {}: {err}",
            header.display()
        )
    });

    let major = parse_define(&contents, "LIBTORRENT_VERSION_MAJOR")
        .unwrap_or_else(|| panic!("missing LIBTORRENT_VERSION_MAJOR in {}", header.display()));
    let minor = parse_define(&contents, "LIBTORRENT_VERSION_MINOR")
        .unwrap_or_else(|| panic!("missing LIBTORRENT_VERSION_MINOR in {}", header.display()));
    let patch = parse_define(&contents, "LIBTORRENT_VERSION_TINY")
        .unwrap_or_else(|| panic!("missing LIBTORRENT_VERSION_TINY in {}", header.display()));

    let required = parse_min_version();
    if (major, minor, patch) < required {
        panic!("libtorrent version {major}.{minor}.{patch} is below required {MIN_VERSION}");
    }
}

fn parse_define(contents: &str, name: &str) -> Option<u32> {
    contents.lines().find_map(|line| {
        let trimmed = line.trim_start();
        if !trimmed.starts_with("#define") {
            return None;
        }
        let mut parts = trimmed.split_whitespace();
        let _ = parts.next()?;
        let key = parts.next()?;
        let value = parts.next()?;
        if key == name {
            value.parse::<u32>().ok()
        } else {
            None
        }
    })
}

fn parse_min_version() -> (u32, u32, u32) {
    let mut parts = MIN_VERSION.split('.');
    let major = parts.next().unwrap_or_default().parse().unwrap_or(0);
    let minor = parts.next().unwrap_or_default().parse().unwrap_or(0);
    let patch = parts.next().unwrap_or_default().parse().unwrap_or(0);
    (major, minor, patch)
}
