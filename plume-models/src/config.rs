use std::env::var;

#[cfg(not(test))]
const DB_NAME: &str =  "plume";
#[cfg(test)]
const DB_NAME: &str = "plume_tests";

pub struct Config {
    pub base_url: String,
    pub use_https: bool,
    pub db_name: &'static str,
    pub database_url: String,
    pub search_index: String,
    pub rocket: RocketConfig,
}

pub struct RocketConfig {
    pub address: String,
    pub port: u16,
    pub secret_key: Option<String>,
    pub form_size: u64,
    pub activity_size: u64
}

lazy_static! {
    pub static ref CONFIG: Config = Config {
        base_url: var("BASE_URL").unwrap_or_else(|_| format!(
                          "127.0.0.1:{}",
                          var("ROCKET_PORT").unwrap_or_else(|_| "8000".to_owned()
                      ))),
        use_https: var("USE_HTTPS").map(|val| val =="1").unwrap_or(true),
        db_name: DB_NAME,
        #[cfg(feature = "postgres")]
        database_url: var("DATABASE_URL").unwrap_or_else(|_| format!(
                "postgres://plume:plume@localhost/{}",
                DB_NAME
                )),
        #[cfg(feature = "sqlite")]
        database_url: var("DATABASE_URL").unwrap_or_else(|_| format!(
                "{}.sqlite",
                DB_NAME
                )),
        search_index: var("SEARCH_INDEX").unwrap_or_else(|_| "search_index".to_owned()),
        rocket: RocketConfig {
            address: var("ROCKET_ADDRESS").unwrap_or_else(|_| "localhost".to_owned()),
            port: var("ROCKET_PORT").ok().map(|s| s.parse::<u16>().unwrap()).unwrap_or(7878),
            secret_key: var("ROCKET_SECRET_KEY").ok(),
            form_size: var("FORM_SIZE").unwrap_or_else(|_| "32".to_owned()).parse::<u64>().unwrap(),
            activity_size: var("ACTIVITY_SIZE").unwrap_or_else(|_| "1024".to_owned()).parse::<u64>().unwrap(),
        }
    };
}
