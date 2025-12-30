use std::env;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

const MIN_VERSION: &str = "2.0.10";

fn main() {
    if let Err(err) = try_main() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn try_main() -> Result<(), BuildError> {
    println!("cargo:rerun-if-env-changed=LIBTORRENT_INCLUDE_DIR");
    println!("cargo:rerun-if-env-changed=LIBTORRENT_LIB_DIR");
    println!("cargo:rerun-if-env-changed=LIBTORRENT_BUNDLE_DIR");

    let mut bridge = cxx_build::bridge("src/ffi/bridge.rs");
    bridge.flag_if_supported("-std=c++17");
    bridge.file("src/ffi/session.cpp");

    let include_dir = PathBuf::from("src/ffi/include");
    bridge.include(&include_dir);

    if let Some((include, lib)) = bundled_paths() {
        ensure_header_version(&include)?;
        bridge.include(&include);
        println!("cargo:rustc-link-search=native={}", lib.display());
        emit_link_libs(vec!["torrent-rasterbar".to_string()]);
        bridge.compile("revaer-libtorrent");
        emit_reruns();
        return Ok(());
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
        ensure_header_version(include_dir)?;
    } else if lib_dir_override.is_some() {
        return Err(BuildError::MissingIncludeDir);
    }

    if libs.is_empty() {
        let libtorrent = pkg_config::Config::new()
            .atleast_version(MIN_VERSION)
            .probe("libtorrent-rasterbar")
            .map_err(BuildError::PkgConfig)?;
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
    Ok(())
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

fn ensure_header_version(include_dir: &Path) -> Result<(), BuildError> {
    let header = include_dir.join("libtorrent").join("version.hpp");
    let contents =
        fs::read_to_string(&header).map_err(|source| BuildError::ReadHeader { source })?;

    let major =
        parse_define(&contents, "LIBTORRENT_VERSION_MAJOR").ok_or(BuildError::MissingDefine)?;
    let minor =
        parse_define(&contents, "LIBTORRENT_VERSION_MINOR").ok_or(BuildError::MissingDefine)?;
    let patch =
        parse_define(&contents, "LIBTORRENT_VERSION_TINY").ok_or(BuildError::MissingDefine)?;

    let required = parse_min_version()?;
    if (major, minor, patch) < required {
        return Err(BuildError::VersionTooOld);
    }
    Ok(())
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

fn parse_min_version() -> Result<(u32, u32, u32), BuildError> {
    let mut parts = MIN_VERSION.split('.');
    let major = parse_version_part(parts.next()).ok_or(BuildError::InvalidMinVersion)?;
    let minor = parse_version_part(parts.next()).ok_or(BuildError::InvalidMinVersion)?;
    let patch = parse_version_part(parts.next()).ok_or(BuildError::InvalidMinVersion)?;
    Ok((major, minor, patch))
}

fn parse_version_part(value: Option<&str>) -> Option<u32> {
    value.and_then(|part| part.parse::<u32>().ok())
}

#[derive(Debug)]
enum BuildError {
    MissingIncludeDir,
    PkgConfig(pkg_config::Error),
    ReadHeader { source: std::io::Error },
    MissingDefine,
    InvalidMinVersion,
    VersionTooOld,
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BuildError::MissingIncludeDir => write!(f, "libtorrent include directory missing"),
            BuildError::PkgConfig(_) => write!(f, "libtorrent pkg-config probe failed"),
            BuildError::ReadHeader { .. } => write!(f, "libtorrent version header read failed"),
            BuildError::MissingDefine => write!(f, "libtorrent version header missing field"),
            BuildError::InvalidMinVersion => write!(f, "invalid libtorrent minimum version"),
            BuildError::VersionTooOld => write!(f, "libtorrent version is too old"),
        }
    }
}

impl Error for BuildError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            BuildError::PkgConfig(err) => Some(err),
            BuildError::ReadHeader { source, .. } => Some(source),
            _ => None,
        }
    }
}
