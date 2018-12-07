extern crate ructe;
extern crate rocket_i18n;
use ructe::*;
use std::{env, path::PathBuf};

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let in_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("templates");
    compile_templates(&in_dir, &out_dir).expect("compile templates");

    println!("cargo:rerun-if-changed=po");
    rocket_i18n::update_po("plume", &["de", "en", "fr", "gl", "it", "ja", "nb", "pl", "ru"]);
    rocket_i18n::compile_po("plume", &["de", "en", "fr", "gl", "it", "ja", "nb", "pl", "ru"]);
}
