use clap::{Arg, ArgMatches, Command};

use plume_models::{blogs::Blog, instance::Instance, lists::*, users::User, Connection};

pub fn command() -> Command {
    Command::new("lists")
        .about("Manage lists")
        .subcommand(
            Command::new("new")
                .arg(
                    Arg::new("name")
                        .short('n')
                        .long("name")
                        .action(clap::ArgAction::Set)
                        .help("The name of this list"),
                )
                .arg(
                    Arg::new("type")
                        .short('t')
                        .long("type")
                        .action(clap::ArgAction::Set)
                        .help(
                            r#"The type of this list (one of "user", "blog", "word" or "prefix")"#,
                        ),
                )
                .arg(
                    Arg::new("user")
                        .short('u')
                        .long("user")
                        .action(clap::ArgAction::Set)
                        .help("Username of whom this list is for. Empty for an instance list"),
                )
                .about("Create a new list"),
        )
        .subcommand(
            Command::new("delete")
                .arg(
                    Arg::new("name")
                        .short('n')
                        .long("name")
                        .action(clap::ArgAction::Set)
                        .help("The name of the list to delete"),
                )
                .arg(
                    Arg::new("user")
                        .short('u')
                        .long("user")
                        .action(clap::ArgAction::Set)
                        .help("Username of whom this list was for. Empty for instance list"),
                )
                .arg(
                    Arg::new("yes")
                        .short('y')
                        .long("yes")
                        .action(clap::ArgAction::SetTrue)
                        .help("Confirm the deletion"),
                )
                .about("Delete a list"),
        )
        .subcommand(
            Command::new("add")
                .arg(
                    Arg::new("name")
                        .short('n')
                        .long("name")
                        .action(clap::ArgAction::Set)
                        .help("The name of the list to add an element to"),
                )
                .arg(
                    Arg::new("user")
                        .short('u')
                        .long("user")
                        .action(clap::ArgAction::Set)
                        .help("Username of whom this list is for. Empty for instance list"),
                )
                .arg(
                    Arg::new("value")
                        .short('v')
                        .long("value")
                        .action(clap::ArgAction::Set)
                        .help("The value to add"),
                )
                .about("Add element to a list"),
        )
        .subcommand(
            Command::new("rm")
                .arg(
                    Arg::new("name")
                        .short('n')
                        .long("name")
                        .action(clap::ArgAction::Set)
                        .help("The name of the list to remove an element from"),
                )
                .arg(
                    Arg::new("user")
                        .short('u')
                        .long("user")
                        .action(clap::ArgAction::Set)
                        .help("Username of whom this list is for. Empty for instance list"),
                )
                .arg(
                    Arg::new("value")
                        .short('v')
                        .long("value")
                        .action(clap::ArgAction::Set)
                        .help("The value to remove"),
                )
                .about("Remove element from list"),
        )
}

pub async fn run(mut args: ArgMatches, conn: &mut Connection) {
    if let Some((c, a)) = args.remove_subcommand() {
        match c.as_str() {
            "new" => new(a, conn),
            "delete" => delete(a, conn),
            "add" => add(a, conn).await,
            "rm" => rm(a, conn).await,
            _ => command().print_help().unwrap(),
        }
    } else {
      println!("Unknown subcommand");
    }
}

fn get_list_identifier(args: &mut ArgMatches) -> (String, Option<String>) {
    let name = args
        .remove_one::<String>("name")
        .expect("No name provided for the list");
    let user = args.remove_one::<String>("user");
    (name, user)
}

fn get_list_type(args: &mut ArgMatches) -> ListType {
    let typ = args
        .remove_one::<String>("type")
        .expect("No name type for the list");

    match typ.as_str() {
        "user" => ListType::User,
        "blog" => ListType::Blog,
        "word" => ListType::Word,
        "prefix" => ListType::Prefix,
        _ => panic!("Invalid list type: {}", typ),
    }
}

fn get_value(args: &mut ArgMatches) -> String {
    args.remove_one::<String>("value").expect("No query provided")
}

fn resolve_user(username: &str, conn: &mut Connection) -> User {
    let instance = Instance::get_local_uncached(conn).expect("Failed to load local instance");

    User::find_by_name(conn, username, instance.id).expect("User not found")
}

fn new(mut args: ArgMatches, conn: &mut Connection) {
    let (name, user) = get_list_identifier(&mut args);
    let typ = get_list_type(&mut args);

    let user = user.map(|user| resolve_user(&user, conn));

    List::new(conn, &name, user.as_ref(), typ).expect("failed to create list");
}

fn delete(mut args: ArgMatches, conn: &mut Connection) {
    let (name, user) = get_list_identifier(&mut args);

    if !args.contains_id("yes") {
        panic!("Warning, this operation is destructive. Add --yes to confirm you want to do it.")
    }

    let user = user.map(|user| resolve_user(&user, conn));

    let list =
        List::find_for_user_by_name(conn, user.map(|u| u.id), &name).expect("list not found");

    list.delete(conn).expect("Failed to update list");
}

async fn add(mut args: ArgMatches, conn: &mut Connection) {
    let (name, user) = get_list_identifier(&mut args);
    let value = get_value(&mut args);

    let user = user.map(|user| resolve_user(&user, conn));

    let list =
        List::find_for_user_by_name(conn, user.map(|u| u.id), &name).expect("list not found");

    match list.kind() {
        ListType::Blog => {
            let blog_id = Blog::find_by_fqn(conn, &value).await.expect("unknown blog").id;
            if !list.contains_blog(conn, blog_id).unwrap() {
                list.add_blogs(conn, &[blog_id]).unwrap();
            }
        }
        ListType::User => {
            let user_id = User::find_by_fqn(conn, &value).await.expect("unknown user").id;
            if !list.contains_user(conn, user_id).unwrap() {
                list.add_users(conn, &[user_id]).unwrap();
            }
        }
        ListType::Word => {
            if !list.contains_word(conn, &value).unwrap() {
                list.add_words(conn, &[&value]).unwrap();
            }
        }
        ListType::Prefix => {
            if !list.contains_prefix(conn, &value).unwrap() {
                list.add_prefixes(conn, &[&value]).unwrap();
            }
        }
    }
}

async fn rm(mut args: ArgMatches, conn: &mut Connection) {
    let (name, user) = get_list_identifier(&mut args);
    let value = get_value(&mut args);

    let user = user.map(|user| resolve_user(&user, conn));

    let list =
        List::find_for_user_by_name(conn, user.map(|u| u.id), &name).expect("list not found");

    match list.kind() {
        ListType::Blog => {
            let blog_id = Blog::find_by_fqn(conn, &value).await.expect("unknown blog").id;
            let mut blogs = list.list_blogs(conn).unwrap();
            if let Some(index) = blogs.iter().position(|b| b.id == blog_id) {
                blogs.swap_remove(index);
                let blogs = blogs.iter().map(|b| b.id).collect::<Vec<_>>();
                list.set_blogs(conn, &blogs).unwrap();
            }
        }
        ListType::User => {
            let user_id = User::find_by_fqn(conn, &value).await.expect("unknown user").id;
            let mut users = list.list_users(conn).unwrap();
            if let Some(index) = users.iter().position(|u| u.id == user_id) {
                users.swap_remove(index);
                let users = users.iter().map(|u| u.id).collect::<Vec<_>>();
                list.set_users(conn, &users).unwrap();
            }
        }
        ListType::Word => {
            let mut words = list.list_words(conn).unwrap();
            if let Some(index) = words.iter().position(|w| *w == value) {
                words.swap_remove(index);
                let words = words.iter().map(String::as_str).collect::<Vec<_>>();
                list.set_words(conn, &words).unwrap();
            }
        }
        ListType::Prefix => {
            let mut prefixes = list.list_prefixes(conn).unwrap();
            if let Some(index) = prefixes.iter().position(|p| *p == value) {
                prefixes.swap_remove(index);
                let prefixes = prefixes.iter().map(String::as_str).collect::<Vec<_>>();
                list.set_prefixes(conn, &prefixes).unwrap();
            }
        }
    }
}
