use colored::Colorize;
use diesel::{pg::PgConnection, r2d2::{ConnectionManager, Pool}};
use dotenv::dotenv;
use std::fs;
use std::io;
use std::process::{exit, Command};
use rpassword;

use DB_URL;
use db_conn::DbConn;
use models::instance::*;
use models::users::*;

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
                    run_setup(Some(db_conn));
                }
            }
            Err(_) => panic!("Couldn't connect to database")
        }
        pool
    } else {
        run_setup(None);
        init_pool().unwrap()
    }
}

fn run_setup(conn: Option<DbConn>) {
    println!("\n\n");
    println!("{}\n{}\n{}\n\n{}",
        "Welcome in the Plume setup tool.".magenta(),
        "It will help you setup your new instance, by asking you a few questions.".magenta(),
        "Then you'll be able to enjoy Plume!".magenta(),
        "First let's check that you have all the required dependencies. Press Enter to start."
    );
    read_line();
    check_native_deps();
    setup_type(conn.expect("Couldn't connect to the Plume database"));

    println!("{}\n{}\n{}",
        "Your Plume instance is now ready to be used.".magenta(),
        "We hope you will enjoy it.".magenta(),
        "If you ever encounter a problem, feel free to report it at https://github.com/Plume-org/Plume/issues/".magenta(),
    );
}

fn setup_type(conn: DbConn) {
    println!("\nDo you prefer a simple setup, or to customize everything?\n");
    println!("  1 - Simple setup");
    println!("  2 - Complete setup");
    match read_line().as_ref() {
        "Simple" | "simple" | "s" | "S" |
        "1" => quick_setup(conn),
        "Complete" | "complete" | "c" | "C" |
        "2" => complete_setup(conn),
        x => {
            println!("Invalid choice. Choose between '1' or '2'. {}", x);
            setup_type(conn);
        }
    }
}

fn quick_setup(conn: DbConn) {
    println!("What is your instance domain?");
    let domain = read_line();
    write_to_dotenv("BASE_URL", domain.clone());

    println!("\nWhat is your instance name?");
    let name = read_line();

    let instance = Instance::insert(&*conn, NewInstance {
        public_domain: domain,
        name: name,
        local: true
    });

    println!("{}\n", "  ✔️ Your instance was succesfully created!".green());

    create_admin(instance, conn);
}

fn complete_setup(conn: DbConn) {
    // TODO
    quick_setup(conn);
}

fn create_admin(instance: Instance, conn: DbConn) {
    println!("{}\n\n", "You are now about to create your admin account".magenta());

    println!("What is your username? (default: admin)");
    let name = read_line_or("admin");

    println!("What is your email?");
    let email = read_line();

    println!("What is your password?");
    let password = rpassword::read_password().expect("Couldn't read your password.");

    User::insert(&*conn, NewUser::new_local(
        name.clone(),
        name,
        true,
        format!("Admin of {}", instance.name),
        email,
        User::hash_pass(password),
        instance.id
    )).update_boxes(&*conn);

    println!("{}\n", "  ✔️ Your account was succesfully created!".green());
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
        println!("{}", "  ✔️ All native dependencies are present.".green())
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
    input.retain(|c| c != '\n');
    input
}

fn read_line_or(or: &str) -> String {
    let input = read_line();
    if input.len() == 0 {
        or.to_string()
    } else {
        input
    }
}

fn write_to_dotenv(var: &'static str, val: String) {
    fs::write(".env", format!("{}\n{}={}", fs::read_to_string(".env").expect("Unable to read .env"), var, val)).expect("Unable to write .env");
}
