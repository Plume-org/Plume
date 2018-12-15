extern crate ructe;
extern crate rocket_i18n;
extern crate rsass;
use ructe::*;
use std::{env, fs::File, io::Write, path::PathBuf};

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let in_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("templates");
    compile_templates(&in_dir, &out_dir).expect("compile templates");

    println!("cargo:rerun-if-changed=po");
    rocket_i18n::update_po("plume", &["de", "en", "fr", "gl", "it", "ja", "nb", "pl", "ru"]);
    rocket_i18n::compile_po("plume", &["de", "en", "fr", "gl", "it", "ja", "nb", "pl", "ru"]);

    println!("cargo:rerun-if-changed=static/css");
    let mut out = File::create("static/css/main.css").expect("Couldn't create main.css");
    println!("annana");
    out.write_all(
        &rsass::compile_scss_file("static/css/main.scss".as_ref(), rsass::OutputStyle::Compressed)
            .expect("Error during SCSS compilation")
    ).expect("Couldn't write CSS output");
}
