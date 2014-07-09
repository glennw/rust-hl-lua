#![crate_type = "dylib"]
#![feature(phase)]

#[phase(plugin)]
extern crate rust_hl_lua_modules;

extern crate rust_hl_lua;
extern crate libc;

fn function1(a: int, b: int) -> int { a + b }
fn function2(a: int) -> int { a + 5 }

lua_module!("mylib", "")