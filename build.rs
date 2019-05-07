extern crate rsass;
extern crate ructe;
use ructe::*;
use std::process::{Command, Stdio};
use std::{env, fs::*, io::Write, path::PathBuf};

fn compute_static_hash() -> String {
    //"find static/ -type f ! -path 'static/media/*' | sort | xargs stat --printf='%n %Y\n' | openssl dgst -r"

    let find = Command::new("find")
        .args(&["static/", "-type", "f", "!", "-path", "static/media/*"])
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed find command");

    let sort = Command::new("sort")
        .stdin(find.stdout.unwrap())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed sort command");

    let xargs = Command::new("xargs")
        .args(&["stat", "--printf='%n %Y\n'"])
        .stdin(sort.stdout.unwrap())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed xargs command");

    let mut sha = Command::new("openssl")
        .args(&["dgst", "-r"])
        .stdin(xargs.stdout.unwrap())
        .output()
        .expect("failed openssl command");

    sha.stdout.resize(64, 0);
    String::from_utf8(sha.stdout).unwrap()
}

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let in_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("templates");
    compile_templates(&in_dir, &out_dir).expect("compile templates");

    println!("cargo:rerun-if-changed=static/css");
    let mut out = File::create("static/css/main.css").expect("Couldn't create main.css");
    out.write_all(
        &rsass::compile_scss_file(
            "static/css/main.scss".as_ref(),
            rsass::OutputStyle::Compressed,
        )
        .expect("Error during SCSS compilation"),
    )
    .expect("Couldn't write CSS output");

    let cache_id = &compute_static_hash()[..8];
    println!("cargo:rerun-if-changed=target/deploy/plume-front.wasm");
    copy("target/deploy/plume-front.wasm", "static/plume-front.wasm")
        .and_then(|_| read_to_string("target/deploy/plume-front.js"))
        .and_then(|js| {
            write(
                "static/plume-front.js",
                js.replace(
                    "\"plume-front.wasm\"",
                    &format!("\"/static/cached/{}/plume-front.wasm\"", cache_id),
                ),
            )
        })
        .ok();

    println!("cargo:rustc-env=CACHE_ID={}", cache_id)
}
