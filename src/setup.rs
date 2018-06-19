use colored::Colorize;
use diesel::{pg::PgConnection, r2d2::{ConnectionManager, Pool}};
use dotenv::dotenv;
use std::io;
use std::process::{exit, Command};

use DB_URL;
use db_conn::DbConn;
use models::instance::Instance;

type PgPool = Pool<ConnectionManager<PgConnection>>;

/// Initializes a database pool.
fn init_pool() -> Option<PgPool> {
    dotenv().ok();

    let manager = ConnectionManager::<PgConnection>::new(DB_URL.as_str());
    Pool::new(manager).ok()
}

pub fn check() -> PgPool {
    if let Some(pool) = init_pool() {
        match pool.get() {
            Ok(conn) => {
                let db_conn = DbConn(conn);
                if Instance::get_local(&*db_conn).is_none() {
                    run_setup();
                }
            }
            Err(_) => panic!("Couldn't connect to database")
        }
        pool
    } else {
        run_setup();
        init_pool().unwrap()
    }
}

fn run_setup() {
    println!("\n\n");
    println!("{}\n{}\n{}\n\n{}",
        "Welcome in the Plume setup tool.".magenta(),
        "It will help you setup your new instance, by asking you a few questions.".magenta(),
        "Then you'll be able to enjoy Plume!".magenta(),
        "First let's check that you have all the required dependencies. Press Enter to start."
    );
    read_line();
    check_native_deps();
}

fn check_native_deps() {
    let mut not_found = Vec::new();
    if !try_run("psql") {
        not_found.push(("PostgreSQL", "sudo apt install postgres"));
    }
    if !try_run("gettext") {
        not_found.push(("GetText", "sudo apt install gettext"))
    }
    if !try_run("diesel") {
        not_found.push(("Diesel CLI", "cargo install diesel_cli"))
    }

    if not_found.len() > 0 {
        println!("{}\n", "Some native dependencies are missing:".red());
        for (dep, install) in not_found.into_iter() {
            println!("{}", format!("  - {} (can be installed with `{}`, on Debian based distributions)", dep, install).red())
        }
        println!("\nRetry once you have installed them.");
        exit(1);
    } else {
        println!("{}", "✔️ All native dependencies are present".green())
    }
}

fn try_run(command: &'static str) -> bool {
    Command::new(command)
        .output()
        .is_ok()
}

fn read_line() -> String {
    let mut input = String::new();
    io::stdin().read_line(&mut input).expect("Unable to read line");
    input
}
