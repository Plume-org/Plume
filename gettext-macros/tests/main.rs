use gettext_macros::*;

init_i18n!("test", fr, en, de, ja);

#[test]
fn main() {
    let msgid1 = t!("This should be translated");
    let msgid2 = t!(
        "This should also be translated",
        "And also has a plural version"
    );
    let cat = get_i18n();
    i18n!(cat, msgid1);
    i18n!(cat, msgid2.0, msgid2.1; 42);
    let x = i18n!(cat, "Hello");
    let b = i18n!(cat, "Singular", "Plural"; 0);
    i18n!(cat, "Singular", "Plural {}"; 0);
    i18n!(cat, context = "Test context", "Hello");
    i18n!(cat, context = "Test context (plural)", "Hello", "Plural"; 2);
    i18n!(cat, context = "Test context (format)", "Hello {}"; "world");
    println!("{} {}", x, b);
    println!("{}", i18n!(cat, "Woohoo, it {}"; "works"));
    println!(i18n_domain!());

    let v = vec![1, 2, 3];
    i18n!(cat, "Test complex formatting {}, {}", "aaahh"; v[0], v[2]);
}

compile_i18n!();

fn get_i18n() -> gettext::Catalog {
    include_i18n!()[0].1.clone()
}
