use std::env::var;
use rocket::Config as RocketConfig;
use rocket::config::Limits;

#[cfg(not(test))]
const DB_NAME: &str =  "plume";
#[cfg(test)]
const DB_NAME: &str = "plume_tests";

pub struct Config {
    pub base_url: String,
    pub db_name: &'static str,
    pub database_url: String,
    pub search_index: String,
    pub rocket: Result<RocketConfig, RocketError>,
}

#[derive(Debug,Clone)]
pub enum RocketError {
    InvalidEnv,
    InvalidAddress,
    InvalidSecretKey,
}

fn get_rocket_config() -> Result<RocketConfig, RocketError> {
    let mut c = RocketConfig::active().map_err(|_| RocketError::InvalidEnv)?;

    let address = var("ROCKET_ADDRESS").unwrap_or_else(|_| "localhost".to_owned());
    let port = var("ROCKET_PORT").ok().map(|s| s.parse::<u16>().unwrap()).unwrap_or(7878);
    let secret_key = var("ROCKET_SECRET_KEY").map_err(|_| RocketError::InvalidSecretKey)?;
    let form_size =  var("FORM_SIZE").unwrap_or_else(|_| "32".to_owned()).parse::<u64>().unwrap();
    let activity_size = var("ACTIVITY_SIZE").unwrap_or_else(|_| "1024".to_owned()).parse::<u64>().unwrap();

    c.set_address(address).map_err(|_| RocketError::InvalidAddress)?;
    c.set_port(port);
    c.set_secret_key(secret_key).map_err(|_| RocketError::InvalidSecretKey) ?;

    c.set_limits(Limits::new()
                      .limit("forms", form_size * 1024)
                      .limit("json", activity_size * 1024));

    Ok(c)
}

lazy_static! {
    pub static ref CONFIG: Config = Config {
        base_url: var("BASE_URL").unwrap_or_else(|_| format!(
                          "127.0.0.1:{}",
                          var("ROCKET_PORT").unwrap_or_else(|_| "8000".to_owned()
                      ))),
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
        rocket: get_rocket_config()
    };
}
