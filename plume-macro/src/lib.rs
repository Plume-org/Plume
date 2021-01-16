#![recursion_limit = "128"]

#[macro_use]
extern crate quote;

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use std::fs::{read_dir, File};
use std::io::Read;
use std::path::Path;
use std::str::FromStr;

#[proc_macro]
pub fn import_migrations(input: TokenStream) -> TokenStream {
    assert!(input.is_empty());
    let migration_dir = if cfg!(feature = "postgres") {
        "migrations/postgres"
    } else if cfg!(feature = "sqlite") {
        "migrations/sqlite"
    } else {
        "migrations"
    };
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|path| path.join(migration_dir).is_dir() || path.join(".git").exists())
        .expect("migrations dir not found")
        .join(migration_dir);
    let mut files = read_dir(path)
        .unwrap()
        .map(|dir| dir.unwrap())
        .filter(|dir| dir.file_type().unwrap().is_dir())
        .map(|dir| dir.path())
        .collect::<Vec<_>>();
    files.sort_unstable();
    let migrations = files
        .into_iter()
        .map(|path| {
            let mut up = path.clone();
            let mut down = path.clone();
            up.push("up.sql");
            down.push("down.sql");
            let mut up_sql = String::new();
            let mut down_sql = String::new();
            File::open(up).unwrap().read_to_string(&mut up_sql).unwrap();
            File::open(down)
                .unwrap()
                .read_to_string(&mut down_sql)
                .unwrap();
            let name = path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .chars()
                .filter(char::is_ascii_digit)
                .take(14)
                .collect::<String>();
            (name, up_sql, down_sql)
        })
        .collect::<Vec<_>>();
    let migrations_name = migrations.iter().map(|m| &m.0).collect::<Vec<_>>();
    let migrations_up = migrations
        .iter()
        .map(|m| m.1.as_str())
        .map(file_to_migration)
        .collect::<Vec<_>>();
    let migrations_down = migrations
        .iter()
        .map(|m| m.2.as_str())
        .map(file_to_migration)
        .collect::<Vec<_>>();

    /*
    enum Action {
        Sql(&'static str),
        Function(&'static Fn(&Connection, &Path) -> Result<()>)
    }*/

    quote!(
        ImportedMigrations(
            &[#(ComplexMigration{name: #migrations_name, up: #migrations_up, down: #migrations_down}),*]
            )
    ).into()
}

fn file_to_migration(file: &str) -> TokenStream2 {
    let mut sql = true;
    let mut acc = String::new();
    let mut actions = vec![];
    for line in file.lines() {
        if sql {
            if let Some(acc_str) = line.strip_prefix("--#!") {
                if !acc.trim().is_empty() {
                    actions.push(quote!(Action::Sql(#acc)));
                }
                sql = false;
                acc = acc_str.to_string();
                acc.push('\n');
            } else if line.starts_with("--") {
                continue;
            } else {
                acc.push_str(line);
                acc.push('\n');
            }
        } else if let Some(acc_str) = line.strip_prefix("--#!") {
            acc.push_str(&acc_str);
            acc.push('\n');
        } else if line.starts_with("--") {
            continue;
        } else {
            let func: TokenStream2 = trampoline(TokenStream::from_str(&acc).unwrap().into());
            actions.push(quote!(Action::Function(&#func)));
            sql = true;
            acc = line.to_string();
            acc.push('\n');
        }
    }
    if !acc.trim().is_empty() {
        if sql {
            actions.push(quote!(Action::Sql(#acc)));
        } else {
            let func: TokenStream2 = trampoline(TokenStream::from_str(&acc).unwrap().into());
            actions.push(quote!(Action::Function(&#func)));
        }
    }

    quote!(
        &[#(#actions),*]
    )
}

/// Build a trampoline to allow reference to closure from const context
fn trampoline(closure: TokenStream2) -> TokenStream2 {
    quote! {
        {
            fn trampoline<'a, 'b>(conn: &'a Connection, path: &'b Path) -> Result<()> {
                (#closure)(conn, path)
            }
            trampoline
        }
    }
}
