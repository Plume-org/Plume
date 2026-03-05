use clap::{Arg, ArgMatches, Command};

use plume_models::{search::Searcher, Connection, CONFIG};
use std::fs::{read_dir, remove_file};
use std::io::ErrorKind;
use std::path::Path;

pub fn command() -> Command {
    Command::new("search")
        .about("Manage search index")
        .subcommand(
            Command::new("init")
                .arg(
                    Arg::new("path")
                        .short('p')
                        .long("path")
                        .action(clap::ArgAction::Set)
                        .required(false)
                        .help("Path to Plume's working directory"),
                )
                .arg(
                    Arg::new("force")
                        .short('f')
                        .long("force")
                        .action(clap::ArgAction::SetTrue)
                        .help("Ignore already using directory"),
                )
                .about("Initialize Plume's internal search engine"),
        )
        .subcommand(
            Command::new("refill")
                .arg(
                    Arg::new("path")
                        .short('p')
                        .long("path")
                        .action(clap::ArgAction::Set)
                        .required(false)
                        .help("Path to Plume's working directory"),
                )
                .about("Regenerate Plume's search index"),
        )
        .subcommand(
            Command::new("unlock")
                .arg(
                    Arg::new("path")
                        .short('p')
                        .long("path")
                        .action(clap::ArgAction::Set)
                        .required(false)
                        .help("Path to Plume's working directory"),
                )
                .about("Release lock on search directory"),
        )
}

pub fn run(mut args: ArgMatches, conn: &mut Connection) {
    args.remove_subcommand().map(|(c, a)| {
        match c.as_str() {
            "init" => init(a, conn),
            "refill" => refill(a, conn, None),
            "unlock" => unlock(a),
            _ => command().print_help().unwrap(),
        }
    }).unwrap_or_else(|| println!("Unknown subcommand") )
}

fn init(mut args: ArgMatches, conn: &mut Connection) {
    let path = args
        .remove_one::<String>("path")
        .map(|p| Path::new(&p).join("search_index"))
        .unwrap_or_else(|| Path::new(&CONFIG.search_index).to_path_buf());

    let force = args.contains_id("force");

    let can_do = match read_dir(path.clone()) {
        // try to read the directory specified
        Ok(mut contents) => contents.next().is_none(),
        Err(e) => {
            if e.kind() == ErrorKind::NotFound {
                true
            } else {
                panic!("Error while initialising search index : {}", e);
            }
        }
    };
    if can_do || force {
        let searcher = Searcher::create(&path, &CONFIG.search_tokenizers).unwrap();
        refill(args, conn, Some(searcher));
    } else {
        eprintln!(
            "Can't create new index, {} exist and is not empty",
            path.to_str().unwrap()
        );
    }
}

fn refill(mut args: ArgMatches, conn: &mut Connection, searcher: Option<Searcher>) {
    let path = match args.remove_one::<String>("path") {
        Some(path) => Path::new(&path).join("search_index"),
        None => Path::new(&CONFIG.search_index).to_path_buf(),
    };
    let searcher =
        searcher.unwrap_or_else(|| Searcher::open(&path, &CONFIG.search_tokenizers).unwrap());

    searcher.fill(conn).expect("Couldn't import post");
    println!("Commiting result");
    searcher.commit();
}

fn unlock(mut args: ArgMatches) {
    let path = match args.remove_one::<String>("path") {
        None => CONFIG.search_index.clone(),
        Some(x) => x,
    };
    let meta = Path::new(&path).join(".tantivy-meta.lock");
    remove_file(meta).unwrap();
    let writer = Path::new(&path).join(".tantivy-writer.lock");
    remove_file(writer).unwrap();
}
