use std::fs::File;
use std::io::{BufReader, prelude::*};
use std::path::Path;
use std::process::Command;

fn main() {
    update_po();
    compile_po();
    install_po();
}

fn update_po() {
    let pot_path = Path::new("po").join("plume.pot");

    let linguas_file = File::open(Path::new("po").join("LINGUAS")).expect("Couldn't find po/LINGUAS file");
    let linguas = BufReader::new(&linguas_file);
    for lang in linguas.lines() {
        let lang = lang.unwrap();
        let po_path = Path::new("po").join(format!("{}.po", lang.clone()));
        if po_path.exists() && po_path.is_file() {
            println!("Updating {}", lang.clone());
            // Update it
            Command::new("msgmerge")
                .arg(po_path.to_str().unwrap())
                .arg(pot_path.to_str().unwrap())
                .spawn()
                .expect("Couldn't update PO file");
        } else {
            println!("Creating {}", lang.clone());
            // Create it from the template
            Command::new("msginit")
                .arg(format!("--input={}", pot_path.to_str().unwrap()))
                .arg(format!("--output-file={}", po_path.to_str().unwrap()))
                .arg("-l")
                .arg(lang)
                .arg("--no-translator")
                .spawn()
                .expect("Couldn't init PO file");
        }
    }
}

fn compile_po() {}

fn install_po() {}
