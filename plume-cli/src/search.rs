use clap::{Arg, ArgMatches, App, SubCommand};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};

use std::fs::{read_dir, remove_file};
use std::io::ErrorKind;
use std::path::Path;
use plume_models::{
    Connection,
    posts::Post,
    schema::posts,
    search::Searcher,
};

pub fn command<'a, 'b>() -> App<'a, 'b> {
    SubCommand::with_name("search")
        .about("Manage search index")
        .subcommand(SubCommand::with_name("init")
            .arg(Arg::with_name("path")
                .short("p")
                .long("path")
                .takes_value(true)
                .required(true)
                .help("Path to Plume's working directory"))
            .arg(Arg::with_name("force")
                .short("f")
                .long("force")
                .help("Ignore already using directory")
            ).about("Initialize Plume's internal search engine"))
        .subcommand(SubCommand::with_name("refill")
            .arg(Arg::with_name("path")
                .short("p")
                .long("path")
                .takes_value(true)
                .required(true)
                .help("Path to Plume's working directory")
            ).about("Regenerate Plume's search index"))
        .subcommand(SubCommand::with_name("unlock")
            .arg(Arg::with_name("path")
                .short("p")
                .long("path")
                .takes_value(true)
                .required(true)
                .help("Path to Plume's working directory")
            ).about("Release lock on search directory"))
}

pub fn run<'a>(args: &ArgMatches<'a>, conn: &Connection) {
    let conn = conn;
    match args.subcommand() {
        ("init", Some(x)) => init(x, conn),
        ("refill", Some(x)) => refill(x, conn, None),
        ("unlock", Some(x)) => unlock(x),
        _ => println!("Unknown subcommand"),
    }
}

fn init<'a>(args: &ArgMatches<'a>, conn: &Connection) {
    let path = args.value_of("path").unwrap();
    let force = args.is_present("force");
    let path = Path::new(path).join("search_index");

    let can_do = match read_dir(path.clone()) { // try to read the directory specified
        Ok(mut contents) => {
            if contents.next().is_none()  {
                true
            } else {
                false
            }
        },
        Err(e) => if e.kind() == ErrorKind::NotFound {
            true
        } else {
            panic!("Error while initialising search index : {}", e);
        }
    };
    if can_do || force {
        let searcher = Searcher::create(&path).unwrap();
        refill(args, conn, Some(searcher));
    } else {
        eprintln!("Can't create new index, {} exist and is not empty", path.to_str().unwrap());
    }
}

fn refill<'a>(args: &ArgMatches<'a>, conn: &Connection, searcher: Option<Searcher>) {
    let path = args.value_of("path").unwrap();
    let path = Path::new(path).join("search_index");
    let searcher = searcher.unwrap_or_else(|| Searcher::open(&path).unwrap());

    let posts = posts::table
        .filter(posts::published.eq(true))
        .load::<Post>(conn)
        .expect("Post::get_recents: loading error");

    let len = posts.len();
    for (i,post) in posts.iter().enumerate() {
        println!("Importing {}/{} : {}", i+1, len, post.title);
        searcher.update_document(conn, &post);
    }
    println!("Commiting result");
    searcher.commit();
}


fn unlock<'a>(args: &ArgMatches<'a>) {
    let path = args.value_of("path").unwrap();
    let path = Path::new(path).join("search_index/.tantivy-indexer.lock");

    remove_file(path).unwrap();
}


