use crate::search::TokenizerKind as SearchTokenizer;
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
    pub db_max_size: Option<u32>,
    pub db_min_idle: Option<u32>,
    pub search_index: String,
    pub search_tokenizers: SearchTokenizerConfig,
    pub rocket: Result<RocketConfig, RocketError>,
    pub logo: LogoConfig,
    pub default_theme: String,
    pub media_directory: String,
    pub ldap: Option<LdapConfig>,
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

pub struct SearchTokenizerConfig {
    pub tag_tokenizer: SearchTokenizer,
    pub content_tokenizer: SearchTokenizer,
    pub property_tokenizer: SearchTokenizer,
}

impl SearchTokenizerConfig {
    pub fn init() -> Self {
        use SearchTokenizer::*;

        match var("SEARCH_LANG").ok().as_deref() {
            Some("ja") => {
                #[cfg(not(feature = "search-lindera"))]
                panic!("You need build Plume with search-lindera feature, or execute it with SEARCH_TAG_TOKENIZER=ngram and SEARCH_CONTENT_TOKENIZER=ngram to enable Japanese search feature");
                #[cfg(feature = "search-lindera")]
                Self {
                    tag_tokenizer: Self::determine_tokenizer("SEARCH_TAG_TOKENIZER", Lindera),
                    content_tokenizer: Self::determine_tokenizer(
                        "SEARCH_CONTENT_TOKENIZER",
                        Lindera,
                    ),
                    property_tokenizer: Ngram,
                }
            }
            _ => Self {
                tag_tokenizer: Self::determine_tokenizer("SEARCH_TAG_TOKENIZER", Whitespace),
                content_tokenizer: Self::determine_tokenizer("SEARCH_CONTENT_TOKENIZER", Simple),
                property_tokenizer: Ngram,
            },
        }
    }

    fn determine_tokenizer(var_name: &str, default: SearchTokenizer) -> SearchTokenizer {
        use SearchTokenizer::*;

        match var(var_name).ok().as_deref() {
            Some("simple") => Simple,
            Some("ngram") => Ngram,
            Some("whitespace") => Whitespace,
            Some("lindera") => {
                #[cfg(not(feature = "search-lindera"))]
                panic!("You need build Plume with search-lindera feature to use Lindera tokenizer");
                #[cfg(feature = "search-lindera")]
                Lindera
            }
            _ => default,
        }
    }
}

pub struct LdapConfig {
    pub addr: String,
    pub base_dn: String,
    pub tls: bool,
    pub user_name_attr: String,
    pub mail_attr: String,
}

fn get_ldap_config() -> Option<LdapConfig> {
    let addr = var("LDAP_ADDR").ok();
    let base_dn = var("LDAP_BASE_DN").ok();
    match (addr, base_dn) {
        (Some(addr), Some(base_dn)) => {
            let tls = var("LDAP_TLS").unwrap_or_else(|_| "false".to_owned());
            let tls = match tls.as_ref() {
                "1" | "true" | "TRUE" => true,
                "0" | "false" | "FALSE" => false,
                _ => panic!("Invalid LDAP configuration : tls"),
            };
            let user_name_attr = var("LDAP_USER_NAME_ATTR").unwrap_or_else(|_| "cn".to_owned());
            let mail_attr = var("LDAP_USER_MAIL_ATTR").unwrap_or_else(|_| "mail".to_owned());
            Some(LdapConfig {
                addr,
                base_dn,
                tls,
                user_name_attr,
                mail_attr,
            })
        }
        (None, None) => None,
        (_, _) => {
            panic!("Invalid LDAP configuration : both LDAP_ADDR and LDAP_BASE_DN must be set")
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
        db_max_size: var("DB_MAX_SIZE").map_or(None, |s| Some(
            s.parse::<u32>()
                .expect("Couldn't parse DB_MAX_SIZE into u32")
        )),
        db_min_idle: var("DB_MIN_IDLE").map_or(None, |s| Some(
            s.parse::<u32>()
                .expect("Couldn't parse DB_MIN_IDLE into u32")
        )),
        #[cfg(feature = "postgres")]
        database_url: var("DATABASE_URL")
            .unwrap_or_else(|_| format!("postgres://plume:plume@localhost/{}", DB_NAME)),
        #[cfg(feature = "sqlite")]
        database_url: var("DATABASE_URL").unwrap_or_else(|_| format!("{}.sqlite", DB_NAME)),
        search_index: var("SEARCH_INDEX").unwrap_or_else(|_| "search_index".to_owned()),
        search_tokenizers: SearchTokenizerConfig::init(),
        rocket: get_rocket_config(),
        logo: LogoConfig::default(),
        default_theme: var("DEFAULT_THEME").unwrap_or_else(|_| "default-light".to_owned()),
        media_directory: var("MEDIA_UPLOAD_DIRECTORY")
            .unwrap_or_else(|_| "static/media".to_owned()),
        ldap: get_ldap_config(),
    };
}
