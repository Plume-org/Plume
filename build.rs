use ructe::Ructe;
use std::process::{Command, Stdio};
use std::{ffi::OsStr, fs::*, io::Write, path::*};

fn compute_static_hash() -> String {
    //"find static/ -type f ! -path 'static/media/*' | sort | xargs stat -c'%n %Y' | openssl dgst -r"

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
        .args(&["stat", "-c'%n %Y'"])
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
    Ructe::from_env()
        .expect("This must be run with cargo")
        .compile_templates("templates")
        .expect("compile templates");

    compile_themes().expect("Theme compilation error");
    recursive_copy(&Path::new("assets").join("icons"), &Path::new("static"))
        .expect("Couldn't copy icons");
    recursive_copy(&Path::new("assets").join("images"), &Path::new("static"))
        .expect("Couldn't copy images");
    create_dir_all(&Path::new("static").join("media")).expect("Couldn't init media directory");

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

fn compile_themes() -> std::io::Result<()> {
    let input_dir = Path::new("assets").join("themes");
    let output_dir = Path::new("static").join("css");

    let themes = find_themes(input_dir)?;

    for theme in themes {
        compile_theme(&theme, &output_dir)?;
    }

    Ok(())
}

fn find_themes(path: PathBuf) -> std::io::Result<Vec<PathBuf>> {
    let ext = path.extension().and_then(OsStr::to_str);
    if metadata(&path)?.is_dir() {
        Ok(read_dir(&path)?.fold(vec![], |mut themes, ch| {
            if let Ok(ch) = ch {
                if let Ok(mut new) = find_themes(ch.path()) {
                    themes.append(&mut new);
                }
            }
            themes
        }))
    } else if (ext == Some("scss") || ext == Some("sass"))
        && !path.file_name().unwrap().to_str().unwrap().starts_with('_')
    {
        Ok(vec![path.clone()])
    } else {
        Ok(vec![])
    }
}

fn compile_theme(path: &Path, out_dir: &Path) -> std::io::Result<()> {
    let name = path
        .components()
        .skip_while(|c| *c != Component::Normal(OsStr::new("themes")))
        .skip(1)
        .filter_map(|c| {
            c.as_os_str()
                .to_str()
                .unwrap_or_default()
                .splitn(2, '.')
                .next()
        })
        .collect::<Vec<_>>()
        .join("-");

    let dir = path.parent().unwrap();

    let out = out_dir.join(name);
    create_dir_all(&out)?;

    // copy files of the theme that are not scss
    for ch in read_dir(&dir)? {
        recursive_copy(&ch?.path(), &out)?;
    }

    // compile the .scss/.sass file
    let mut out = File::create(out.join("theme.css"))?;
    out.write_all(
        &rsass::compile_scss_file(path, rsass::OutputStyle::Compressed)
            .expect("SCSS compilation error"),
    )?;

    Ok(())
}

fn recursive_copy(path: &Path, out_dir: &Path) -> std::io::Result<()> {
    if metadata(path)?.is_dir() {
        let out = out_dir.join(path.file_name().unwrap());
        create_dir_all(out.clone())?;

        for ch in read_dir(path)? {
            recursive_copy(&ch?.path(), &out)?;
        }
    } else {
        println!("cargo:rerun-if-changed={}", path.display());

        let ext = path.extension().and_then(OsStr::to_str);
        if ext != Some("scss") && ext != Some("sass") {
            copy(path, out_dir.join(path.file_name().unwrap()))?;
        }
    }

    Ok(())
}
