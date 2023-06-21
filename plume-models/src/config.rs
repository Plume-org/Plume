use crate::search::TokenizerKind as SearchTokenizer;
use crate::signups::Strategy as SignupStrategy;
use crate::smtp::{SMTP_PORT, SUBMISSIONS_PORT, SUBMISSION_PORT};
use rocket::config::Limits;
use rocket::Config as RocketConfig;
use std::collections::HashSet;
use std::env::{self, var};

#[cfg(feature = "s3")]
use s3::{Bucket, Region, creds::Credentials};

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
    pub signup: SignupStrategy,
    pub search_index: String,
    pub search_tokenizers: SearchTokenizerConfig,
    pub rocket: Result<RocketConfig, InvalidRocketConfig>,
    pub logo: LogoConfig,
    pub default_theme: String,
    pub media_directory: String,
    pub mail: Option<MailConfig>,
    pub ldap: Option<LdapConfig>,
    pub proxy: Option<ProxyConfig>,
    pub s3: Option<S3Config>,
}

impl Config {
    pub fn proxy(&self) -> Option<&reqwest::Proxy> {
        self.proxy.as_ref().map(|p| &p.proxy)
    }
}

fn string_to_bool(val: &str, name: &str) -> bool {
    match val {
        "1" | "true" | "TRUE" => true,
        "0" | "false" | "FALSE" => false,
        _ => panic!("Invalid configuration: {} is not boolean", name),
    }
}

#[derive(Debug, Clone)]
pub enum InvalidRocketConfig {
    Env,
    Address,
    SecretKey,
}

fn get_rocket_config() -> Result<RocketConfig, InvalidRocketConfig> {
    let mut c = RocketConfig::active().map_err(|_| InvalidRocketConfig::Env)?;

    let address = var("ROCKET_ADDRESS").unwrap_or_else(|_| "localhost".to_owned());
    let port = var("ROCKET_PORT")
        .ok()
        .map(|s| s.parse::<u16>().unwrap())
        .unwrap_or(7878);
    let secret_key = var("ROCKET_SECRET_KEY").map_err(|_| InvalidRocketConfig::SecretKey)?;
    let form_size = var("FORM_SIZE")
        .unwrap_or_else(|_| "128".to_owned())
        .parse::<u64>()
        .unwrap();
    let activity_size = var("ACTIVITY_SIZE")
        .unwrap_or_else(|_| "1024".to_owned())
        .parse::<u64>()
        .unwrap();

    c.set_address(address)
        .map_err(|_| InvalidRocketConfig::Address)?;
    c.set_port(port);
    c.set_secret_key(secret_key)
        .map_err(|_| InvalidRocketConfig::SecretKey)?;

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
            let ext = |path: &str| match path.rsplit_once('.').map(|x| x.1) {
                Some("png") => Some("image/png".to_owned()),
                Some("jpg") | Some("jpeg") => Some("image/jpeg".to_owned()),
                Some("svg") => Some("image/svg+xml".to_owned()),
                Some("webp") => Some("image/webp".to_owned()),
                _ => None,
            };
            let mut custom_icons = env::vars()
                .filter_map(|(var, val)| {
                    var.strip_prefix("PLUME_LOGO_")
                        .map(|size| (size.to_owned(), val))
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

pub struct MailConfig {
    pub server: String,
    pub port: u16,
    pub helo_name: String,
    pub username: String,
    pub password: String,
}

fn get_mail_config() -> Option<MailConfig> {
    Some(MailConfig {
        server: env::var("MAIL_SERVER").ok()?,
        port: env::var("MAIL_PORT").map_or(SUBMISSIONS_PORT, |port| match port.as_str() {
            "smtp" => SMTP_PORT,
            "submissions" => SUBMISSIONS_PORT,
            "submission" => SUBMISSION_PORT,
            number => number
                .parse()
                .expect(r#"MAIL_PORT must be "smtp", "submissions", "submission" or an integer."#),
        }),
        helo_name: env::var("MAIL_HELO_NAME").unwrap_or_else(|_| "localhost".to_owned()),
        username: env::var("MAIL_USER").ok()?,
        password: env::var("MAIL_PASSWORD").ok()?,
    })
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
            let tls = string_to_bool(&tls, "LDAP_TLS");
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

pub struct ProxyConfig {
    pub url: reqwest::Url,
    pub only_domains: Option<HashSet<String>>,
    pub proxy: reqwest::Proxy,
}

fn get_proxy_config() -> Option<ProxyConfig> {
    let url: reqwest::Url = var("PROXY_URL").ok()?.parse().expect("Invalid PROXY_URL");
    let proxy_url = url.clone();
    let only_domains: Option<HashSet<String>> = var("PROXY_DOMAINS")
        .ok()
        .map(|ods| ods.split(',').map(str::to_owned).collect());
    let proxy = if let Some(ref only_domains) = only_domains {
        let only_domains = only_domains.clone();
        reqwest::Proxy::custom(move |url| {
            if let Some(domain) = url.domain() {
                if only_domains.contains(domain)
                    || only_domains
                        .iter()
                        .any(|target| domain.ends_with(&format!(".{}", target)))
                {
                    Some(proxy_url.clone())
                } else {
                    None
                }
            } else {
                None
            }
        })
    } else {
        reqwest::Proxy::all(proxy_url).expect("Invalid PROXY_URL")
    };
    Some(ProxyConfig {
        url,
        only_domains,
        proxy,
    })
}

pub struct S3Config {
    pub bucket: String,
    pub access_key_id: String,
    pub access_key_secret: String,

    // region? If not set, default to us-east-1
    pub region: String,
    // hostname for s3. If not set, default to $region.amazonaws.com
    pub hostname: String,
    // may be useful when using self hosted s3. Won't work with recent AWS buckets
    pub path_style: bool,
    // http or https
    pub protocol: String,

    // download directly from s3 to user, wihout going through Plume. Require public read on bucket
    pub direct_download: bool,
    // use this hostname for downloads, can be used with caching proxy in front of s3 (expected to
    // be reachable through https)
    pub alias: Option<String>,
}

impl S3Config {
    #[cfg(feature = "s3")]
    pub fn get_bucket(&self) -> Bucket {
        let region = Region::Custom {
            region: self.region.clone(),
            endpoint: format!("{}://{}", self.protocol, self.hostname),
        };
        let credentials = Credentials {
            access_key: Some(self.access_key_id.clone()),
            secret_key: Some(self.access_key_secret.clone()),
            security_token: None,
            session_token: None,
            expiration: None,
        };

        let bucket = Bucket::new(&self.bucket, region, credentials).unwrap();
        if self.path_style {
            bucket.with_path_style()
        } else {
            bucket
        }
    }
}

fn get_s3_config() -> Option<S3Config> {
    let bucket = var("S3_BUCKET").ok();
    let access_key_id = var("AWS_ACCESS_KEY_ID").ok();
    let access_key_secret = var("AWS_SECRET_ACCESS_KEY").ok();
    if bucket.is_none() && access_key_id.is_none() && access_key_secret.is_none() {
        return None;
    }

    #[cfg(not(feature = "s3"))]
    panic!("S3 support is not enabled in this build");

    #[cfg(feature = "s3")]
    {
        if bucket.is_none() || access_key_id.is_none() || access_key_secret.is_none() {
            panic!("Invalid S3 configuration: some required values are set, but not others");
        }
        let bucket = bucket.unwrap();
        let access_key_id = access_key_id.unwrap();
        let access_key_secret = access_key_secret.unwrap();

        let region = var("S3_REGION").unwrap_or_else(|_| "us-east-1".to_owned());
        let hostname = var("S3_HOSTNAME").unwrap_or_else(|_| format!("{}.amazonaws.com", region));

        let protocol = var("S3_PROTOCOL").unwrap_or_else(|_| "https".to_owned());
        if protocol != "http" && protocol != "https" {
            panic!("Invalid S3 configuration: invalid protocol {}", protocol);
        }

        let path_style = var("S3_PATH_STYLE").unwrap_or_else(|_| "false".to_owned());
        let path_style = string_to_bool(&path_style, "S3_PATH_STYLE");
        let direct_download = var("S3_DIRECT_DOWNLOAD").unwrap_or_else(|_| "false".to_owned());
        let direct_download = string_to_bool(&direct_download, "S3_DIRECT_DOWNLOAD");

        let alias = var("S3_ALIAS_HOST").ok();

        if direct_download && protocol == "http" && alias.is_none() {
            panic!("S3 direct download is disabled because bucket is accessed through plain HTTP. Use HTTPS or set an alias hostname (S3_ALIAS_HOST).");
        }

        Some(S3Config {
            bucket,
            access_key_id,
            access_key_secret,
            region,
            hostname,
            protocol,
            path_style,
            direct_download,
            alias,
        })
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
        signup: var("SIGNUP").map_or(SignupStrategy::default(), |s| s.parse().unwrap()),
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
        mail: get_mail_config(),
        ldap: get_ldap_config(),
        proxy: get_proxy_config(),
        s3: get_s3_config(),
    };
}
