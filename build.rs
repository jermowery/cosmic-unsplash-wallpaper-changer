// SPDX-License-Identifier: GPL-3.0-only

use std::env;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    // Ensure libxkbcommon.so can be found by the linker on systems where
    // libxkbcommon0 runtime is installed without the libxkbcommon-dev symlink.
    let candidate_dirs = [
        "/usr/lib/x86_64-linux-gnu",
        "/lib/x86_64-linux-gnu",
        "/usr/lib64",
        "/usr/lib",
    ];

    let has_unversioned = candidate_dirs
        .iter()
        .any(|dir| Path::new(dir).join("libxkbcommon.so").exists());

    if !has_unversioned {
        if let Some(out_dir) = env::var_os("OUT_DIR").map(PathBuf::from) {
            for dir in candidate_dirs {
                let versioned = Path::new(dir).join("libxkbcommon.so.0");
                if versioned.exists() {
                    let link_path = out_dir.join("libxkbcommon.so");
                    let _ = std::fs::remove_file(&link_path);
                    if symlink(&versioned, &link_path).is_ok() {
                        println!("cargo:rustc-link-search=native={}", out_dir.display());
                    }
                    break;
                }
            }
        }
    }
}
