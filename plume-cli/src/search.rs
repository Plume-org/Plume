use clap::{App, Arg, ArgMatches, SubCommand};

use plume_models::{search::Searcher, Connection, CONFIG};
use std::fs::{read_dir, remove_file};
use std::io::ErrorKind;
use std::path::Path;

pub fn command<'a, 'b>() -> App<'a, 'b> {
    SubCommand::with_name("search")
        .about("Manage search index")
        .subcommand(
            SubCommand::with_name("init")
                .arg(
                    Arg::with_name("path")
                        .short("p")
                        .long("path")
                        .takes_value(true)
                        .required(false)
                        .help("Path to Plume's working directory"),
                )
                .arg(
                    Arg::with_name("force")
                        .short("f")
                        .long("force")
                        .help("Ignore already using directory"),
                )
                .about("Initialize Plume's internal search engine"),
        )
        .subcommand(
            SubCommand::with_name("refill")
                .arg(
                    Arg::with_name("path")
                        .short("p")
                        .long("path")
                        .takes_value(true)
                        .required(false)
                        .help("Path to Plume's working directory"),
                )
                .about("Regenerate Plume's search index"),
        )
        .subcommand(
            SubCommand::with_name("unlock")
                .arg(
                    Arg::with_name("path")
                        .short("p")
                        .long("path")
                        .takes_value(true)
                        .required(false)
                        .help("Path to Plume's working directory"),
                )
                .about("Release lock on search directory"),
        )
}

pub fn run<'a>(args: &ArgMatches<'a>, conn: &Connection) {
    let conn = conn;
    match args.subcommand() {
        ("init", Some(x)) => init(x, conn),
        ("refill", Some(x)) => refill(x, conn, None),
        ("unlock", Some(x)) => unlock(x),
        ("", None) => command().print_help().unwrap(),
        _ => println!("Unknown subcommand"),
    }
}

fn init<'a>(args: &ArgMatches<'a>, conn: &Connection) {
    let path = args
        .value_of("path")
        .map(|p| Path::new(p).join("search_index"))
        .unwrap_or_else(|| Path::new(&CONFIG.search_index).to_path_buf());
    let force = args.is_present("force");

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

fn refill<'a>(args: &ArgMatches<'a>, conn: &Connection, searcher: Option<Searcher>) {
    let path = args.value_of("path");
    let path = match path {
        Some(path) => Path::new(path).join("search_index"),
        None => Path::new(&CONFIG.search_index).to_path_buf(),
    };
    let searcher =
        searcher.unwrap_or_else(|| Searcher::open(&path, &CONFIG.search_tokenizers).unwrap());

    searcher.fill(conn).expect("Couldn't import post");
    println!("Commiting result");
    searcher.commit();
}

fn unlock(args: &ArgMatches) {
    let path = match args.value_of("path") {
        None => Path::new(&CONFIG.search_index),
        Some(x) => Path::new(x),
    };
    let meta = Path::new(path).join(".tantivy-meta.lock");
    remove_file(meta).unwrap();
    let writer = Path::new(path).join(".tantivy-writer.lock");
    remove_file(writer).unwrap();
}
