// Copyright (C) 2016 Ahmed Charles - acharles@outlook.com
// Distributed under the MIT License.
//    (See accompanying file LICENSE.txt or copy at
//          http://opensource.org/licenses/MIT)

use std::process::Command;

#[cfg(target_os = "macos")]
fn os(cmd: &mut Command) -> &mut Command { cmd.arg("macos") }

#[cfg(target_os = "linux")]
fn os(cmd: &mut Command) -> &mut Command { cmd.arg("linux").arg("MYCFLAGS=-fPIC") }

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn os(cmd: &mut Command) -> &mut Command { cmd.arg("generic") }

fn main() {
    let out_dir = std::env::var("OUT_DIR").unwrap();

    assert!(os(Command::new("make").arg("-C").arg("puc-lua")).status().unwrap().success());
    assert!(Command::new("cp").arg("puc-lua/src/liblua.a").arg(&out_dir).status().unwrap().success());
    assert!(Command::new("cp").arg("puc-lua/src/lua").arg(&out_dir).status().unwrap().success());
    assert!(Command::new("cp").arg("puc-lua/src/lua").arg("puc-lua/src/tests/lua").status().unwrap().success());
    assert!(Command::new("cp").arg("puc-lua/src/luac").arg(&out_dir).status().unwrap().success());
    assert!(Command::new("make").arg("-C").arg("puc-lua").arg("clean").status().unwrap().success());

    println!("cargo:rustc-link-lib=static=lua");
    println!("cargo:rustc-link-search=native={}", out_dir);

    for entry in std::fs::read_dir("puc-lua/src").unwrap() {
        println!("cargo:rerun-if-changed={}", entry.unwrap().path().display());
    }
}
