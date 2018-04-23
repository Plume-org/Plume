use heck::CamelCase;

pub fn make_actor_id(name: String) -> String {
    name.as_str().to_camel_case()
}
