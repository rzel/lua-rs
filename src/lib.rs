// Copyright (C) 2015-2016 Ahmed Charles - acharles@outlook.com
// Distributed under the MIT License.
//    (See accompanying file LICENSE.txt or copy at
//          http://opensource.org/licenses/MIT)

//! Lua port written in Rust.

#![crate_type = "rlib"]
#![crate_type = "dylib"]
#![doc(html_logo_url = "http://www.rust-lang.org/logos/rust-logo-128x128-blk-v2.png",
       html_favicon_url = "http://www.rust-lang.org/favicon.ico")]

extern crate libc;

pub mod ffi;

#[cfg(test)]
mod test {

    #[test]
    fn it_works() {
    }
}
