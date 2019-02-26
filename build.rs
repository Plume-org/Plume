extern crate ructe;
extern crate rsass;
use ructe::*;
use std::{env, fs::*, io::Write, path::PathBuf};

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let in_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("templates");
    compile_templates(&in_dir, &out_dir).expect("compile templates");

    println!("cargo:rerun-if-changed=static/css");
    let mut out = File::create("static/css/main.css").expect("Couldn't create main.css");
    out.write_all(
        &rsass::compile_scss_file("static/css/main.scss".as_ref(), rsass::OutputStyle::Compressed)
            .expect("Error during SCSS compilation")
    ).expect("Couldn't write CSS output");

    copy("target/deploy/plume-front.wasm", "static/plume-front.wasm")
        .and_then(|_| read_to_string("target/deploy/plume-front.js"))
        .and_then(|js| write("static/plume-front.js", js.replace("\"plume-front.wasm\"", "\"/static/plume-front.wasm\""))).ok();
}
