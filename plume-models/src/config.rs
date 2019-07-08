use rocket::config::Limits;
use rocket::Config as RocketConfig;
use std::env::{self, var};

#[cfg(not(test))]
const DB_NAME: &str = "plume";
#[cfg(test)]
const DB_NAME: &str = "plume_tests";

pub struct Config {
    pub base_url: String,
    pub database_url: String,
    pub db_name: &'static str,
    pub search_index: String,
    pub rocket: Result<RocketConfig, RocketError>,
    pub logo: LogoConfig,
    pub ldap: LdapConfig,
}

#[derive(Debug, Clone)]
pub enum RocketError {
    InvalidEnv,
    InvalidAddress,
    InvalidSecretKey,
}

fn get_rocket_config() -> Result<RocketConfig, RocketError> {
    let mut c = RocketConfig::active().map_err(|_| RocketError::InvalidEnv)?;

    let address = var("ROCKET_ADDRESS").unwrap_or_else(|_| "localhost".to_owned());
    let port = var("ROCKET_PORT")
        .ok()
        .map(|s| s.parse::<u16>().unwrap())
        .unwrap_or(7878);
    let secret_key = var("ROCKET_SECRET_KEY").map_err(|_| RocketError::InvalidSecretKey)?;
    let form_size = var("FORM_SIZE")
        .unwrap_or_else(|_| "128".to_owned())
        .parse::<u64>()
        .unwrap();
    let activity_size = var("ACTIVITY_SIZE")
        .unwrap_or_else(|_| "1024".to_owned())
        .parse::<u64>()
        .unwrap();

    c.set_address(address)
        .map_err(|_| RocketError::InvalidAddress)?;
    c.set_port(port);
    c.set_secret_key(secret_key)
        .map_err(|_| RocketError::InvalidSecretKey)?;

    c.set_limits(
        Limits::new()
            .limit("forms", form_size * 1024)
            .limit("json", activity_size * 1024),
    );

    Ok(c)
}

pub struct LogoConfig {
    pub main: String,
    pub favicon: String,
    pub other: Vec<Icon>, //url, size, type
}

#[derive(Serialize)]
pub struct Icon {
    pub src: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sizes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub image_type: Option<String>,
}

impl Icon {
    pub fn with_prefix(&self, prefix: &str) -> Icon {
        Icon {
            src: format!("{}/{}", prefix, self.src),
            sizes: self.sizes.clone(),
            image_type: self.image_type.clone(),
        }
    }
}

impl Default for LogoConfig {
    fn default() -> Self {
        let to_icon = |(src, sizes, image_type): &(&str, Option<&str>, Option<&str>)| Icon {
            src: str::to_owned(src),
            sizes: sizes.map(str::to_owned),
            image_type: image_type.map(str::to_owned),
        };
        let icons = [
            (
                "icons/trwnh/feather/plumeFeather48.png",
                Some("48x48"),
                Some("image/png"),
            ),
            (
                "icons/trwnh/feather/plumeFeather72.png",
                Some("72x72"),
                Some("image/png"),
            ),
            (
                "icons/trwnh/feather/plumeFeather96.png",
                Some("96x96"),
                Some("image/png"),
            ),
            (
                "icons/trwnh/feather/plumeFeather144.png",
                Some("144x144"),
                Some("image/png"),
            ),
            (
                "icons/trwnh/feather/plumeFeather160.png",
                Some("160x160"),
                Some("image/png"),
            ),
            (
                "icons/trwnh/feather/plumeFeather192.png",
                Some("192x192"),
                Some("image/png"),
            ),
            (
                "icons/trwnh/feather/plumeFeather256.png",
                Some("256x256"),
                Some("image/png"),
            ),
            (
                "icons/trwnh/feather/plumeFeather512.png",
                Some("512x512"),
                Some("image/png"),
            ),
            ("icons/trwnh/feather/plumeFeather.svg", None, None),
        ]
        .iter()
        .map(to_icon)
        .collect();

        let custom_main = var("PLUME_LOGO").ok();
        let custom_favicon = var("PLUME_LOGO_FAVICON")
            .ok()
            .or_else(|| custom_main.clone());
        let other = if let Some(main) = custom_main.clone() {
            let ext = |path: &str| match path.rsplitn(2, '.').next() {
                Some("png") => Some("image/png".to_owned()),
                Some("jpg") | Some("jpeg") => Some("image/jpeg".to_owned()),
                Some("svg") => Some("image/svg+xml".to_owned()),
                Some("webp") => Some("image/webp".to_owned()),
                _ => None,
            };
            let mut custom_icons = env::vars()
                .filter_map(|(var, val)| {
                    if var.starts_with("PLUME_LOGO_") {
                        Some((var[11..].to_owned(), val))
                    } else {
                        None
                    }
                })
                .filter_map(|(var, val)| var.parse::<u64>().ok().map(|var| (var, val)))
                .map(|(dim, src)| Icon {
                    image_type: ext(&src),
                    src,
                    sizes: Some(format!("{}x{}", dim, dim)),
                })
                .collect::<Vec<_>>();
            custom_icons.push(Icon {
                image_type: ext(&main),
                src: main,
                sizes: None,
            });
            custom_icons
        } else {
            icons
        };

        LogoConfig {
            main: custom_main
                .unwrap_or_else(|| "icons/trwnh/feather/plumeFeather256.png".to_owned()),
            favicon: custom_favicon.unwrap_or_else(|| {
                "icons/trwnh/feather-filled/plumeFeatherFilled64.png".to_owned()
            }),
            other,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LdapConfig {
    pub url: Option<String>,
    pub bind_dn: Option<String>,
}

impl Default for LdapConfig {
    fn default() -> Self {
        let url = var("LDAP_URL").ok();
        let bind_dn = var("LDAP_BIND_DN").ok();
        if url.is_some() ^ bind_dn.is_some() {
            panic!(
                r#"Invalid configuration :
You must provide both LDAP_URL and LDAP_BIND_DN, or neither"#
            );
        } else {
            LdapConfig { url, bind_dn }
        }
    }
}

lazy_static! {
    pub static ref CONFIG: Config = Config {
        base_url: var("BASE_URL").unwrap_or_else(|_| format!(
            "127.0.0.1:{}",
            var("ROCKET_PORT").unwrap_or_else(|_| "7878".to_owned())
        )),
        db_name: DB_NAME,
        #[cfg(feature = "postgres")]
        database_url: var("DATABASE_URL")
            .unwrap_or_else(|_| format!("postgres://plume:plume@localhost/{}", DB_NAME)),
        #[cfg(feature = "sqlite")]
        database_url: var("DATABASE_URL").unwrap_or_else(|_| format!("{}.sqlite", DB_NAME)),
        search_index: var("SEARCH_INDEX").unwrap_or_else(|_| "search_index".to_owned()),
        rocket: get_rocket_config(),
        logo: LogoConfig::default(),
        ldap: LdapConfig::default(),
    };
}
