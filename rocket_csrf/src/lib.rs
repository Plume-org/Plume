//! # rocket_csrf
//!
//! The `rocket_csrf` crate is a powerful library that provides Cross-Site Request Forgery (CSRF) protection
//! for Rocket web applications. With this library, you can seamlessly integrate CSRF protection into your Rust
//! applications, securing them against CSRF attacks efficiently.
//!
//! # Quick Start
//!
//! Get started with the `rocket_csrf` crate quickly by following these simple steps:
//!
//! 1. Add the `rocket_csrf` crate to your project's `Cargo.toml` file:
//!
//! ```toml
//! [dependencies]
//! rocket_csrf = "0.3.2"
//! ```
//!
//! 2. Import the library into your Rocket application code:
//!
//! ```rust
//! use rocket_csrf::{CsrfConfig, Fairing};
//! ```
//!
//! 3. Initialize the CSRF protection with your custom configuration:
//!
//! ```rust
//! use rocket_csrf::{CsrfConfig, Fairing};
//! use rocket::{Rocket, Build, launch};
//!
//! #[launch]
//! fn rocket() -> Rocket<Build> {
//!     rocket::build()
//!         .attach(Fairing::new(CsrfConfig::default()))
//!         // ...
//! }
//! ```
//!
//! 4. Start securing your routes with CSRF protection using the provided functionality.
//!
//! # Key Features
//!
//! The `rocket_csrf` crate offers a range of features to protect your Rocket web application from CSRF attacks:
//!
//! - **Customizable Configuration**: Configure the CSRF token lifespan, cookie name, and token length according to your needs.
//! - **Automatic CSRF Token Generation**: Automatically generate and manage CSRF tokens for each request.
//! - **Request Verification**: Verify incoming requests for valid CSRF tokens to ensure their authenticity.
//!
//! # Usage
//!
//! ## Configuration
//!
//! You can customize the CSRF protection by configuring the `CsrfConfig` structure. The following code shows how to create a custom configuration:
//!
//! ```rust
//! use rocket_csrf::CsrfConfig;
//! use rocket::time::Duration;
//!
//! fn main() {
//!     let csrf_config = CsrfConfig::default()
//!         .with_lifetime(Some(Duration::hours(2)))
//!         .with_cookie_name("my_csrf_token")
//!         .with_cookie_len(64);
//!     // ...
//! }
//! ```
//!
//! ## Rocket Fairing
//!
//! The `Fairing` struct integrates CSRF protection into your Rocket application. You can attach the fairing during Rocket's setup to enable CSRF protection:
//!
//! ```rust
//! use rocket_csrf::{CsrfConfig, Fairing};
//! use rocket::launch;
//!
//! #[launch]
//! fn rocket() -> _ {
//!     rocket::build()
//!         .attach(Fairing::new(CsrfConfig::default()))
//!         // ...
//! }
//! ```
//!
//! ## Request Handling
//!
//! Use the provided request guards and functionality to handle CSRF tokens within your routes:
//!
//! ```rust
//! use rocket_csrf::{CsrfToken, Fairing};
//! use rocket::{post, http::Status, form::Form, FromForm};
//!
//! #[derive(FromForm)]
//! struct PostData {
//!     authenticity_token: String,
//!     data: String,
//! }
//!
//! #[post("/secure-endpoint", data = "<form>")]
//! async fn secure_endpoint(token: CsrfToken, form: Form<PostData>) -> Result<(), Status> {
//!     // Verify the CSRF token and process the request
//!     if token.verify(&form.authenticity_token).is_ok() {
//!         // Request is valid, continue processing
//!         // ...
//!         Ok(())
//!     } else {
//!         // Handle the CSRF token verification failure
//!         Err(Status::Forbidden)
//!     }
//! }
//!
//! ```

use base64::{engine::general_purpose, Engine as _};
use bcrypt::{hash, verify, BcryptError};
use rand::{distr::StandardUniform, Rng};
use rocket::{
    async_trait, error,
    fairing::{self, Fairing as RocketFairing, Info, Kind},
    http::{
        // ContentType,
        Cookie,
        Status,
    },
    info,
    request::{FromRequest, Outcome},
    response::{Responder, Response},
    time::{Duration, OffsetDateTime},
    Data, Request, Rocket, State,
};
use std::{
    borrow::Cow,
    fmt,
    //io::Cursor
};

// Constants for CSRF handling
const BCRYPT_COST: u32 = 8;
const HEADER_NAME: &str = "X-CSRF-Token";
const _PARAM_NAME: &str = "authenticity_token";
const _PARAM_META_NAME: &str = "csrf-param";
const _TOKEN_META_NAME: &str = "csrf-token";

/// Configuration for Cross-Site Request Forgery (CSRF) protection. It allows you to customize
/// settings related to CSRF token management, including token lifespan, cookie name, and token length.
#[derive(Debug, Clone)]
pub struct CsrfConfig {
    /// The duration for which the CSRF token remains valid.
    lifespan: Option<Duration>,
    /// The name of the CSRF cookie that stores the token.
    cookie_name: Cow<'static, str>,
    /// The length of the CSRF token in bytes.
    cookie_len: usize,
}

impl Default for CsrfConfig {
    /// Creates a default CsrfConfig with the following default settings:
    /// - Lifespan: 1 day
    /// - Cookie Name: "csrf_token"
    /// - Token Length: 32 bytes
    ///
    /// This function returns a new CsrfConfig instance with the default settings.
    fn default() -> Self {
        Self {
            lifespan: Some(Duration::days(1)),
            cookie_name: "csrf_token".into(),
            cookie_len: 32,
        }
    }
}

impl CsrfConfig {
    /// Sets the lifespan of the CSRF token cookie.
    /// # Arguments
    /// * `Option<rocket::Duration>` - The duration for which the CSRF token remains valid.
    ///
    /// This function modifies the CsrfConfig instance by setting the token lifespan to the
    /// specified duration.
    pub fn with_lifetime(mut self, time: Option<Duration>) -> Self {
        self.lifespan = time;
        self
    }

    /// Sets the name of the CSRF cookie.
    /// # Arguments
    /// * `name` - The name of the CSRF cookie.
    ///
    /// This function modifies the CsrfConfig instance by setting the cookie name to the provided name.
    pub fn with_cookie_name(mut self, name: impl Into<Cow<'static, str>>) -> Self {
        self.cookie_name = name.into();
        self
    }

    /// Sets the length of the CSRF token.
    /// # Arguments
    /// * `length` - The desired length of the CSRF token in bytes.
    ///
    /// This function modifies the CsrfConfig instance by setting the token length to the specified value.
    /// It is important to ensure that the token length is 16 bytes or larger.
    pub fn with_cookie_len(mut self, length: usize) -> Self {
        self.cookie_len = length;
        self
    }
}

/// Rocket fairing for CSRF protection. This fairing is responsible for handling and managing CSRF tokens
/// during Rocket application runtime.
pub struct Fairing {
    config: CsrfConfig,
}

impl Default for Fairing {
    /// Creates a default Fairing with the default CsrfConfig settings. This fairing can be used to
    /// automatically manage CSRF tokens in a Rocket application.
    ///
    /// This function returns a new Fairing instance with the default CsrfConfig settings.
    fn default() -> Self {
        Self::new(CsrfConfig::default())
    }
}

/// Define custom methods and functions for the `Fairing` type itself.
/// It is like defining methods in a blueprint or abstract class.
impl Fairing {
    /// Creates a new CSRF protection fairing with the provided configuration.
    /// # Arguments
    /// * `config` - The configuration specifying how CSRF tokens should be managed.
    ///
    /// This function creates a new Fairing instance with the given configuration, allowing for
    /// customization of CSRF token management in a Rocket application.
    pub fn new(config: CsrfConfig) -> Self {
        Self { config }
    }
}

/// Structure to hold a CSRF token. This token can be used for generating authenticity tokens
/// and verifying the authenticity of incoming requests.
#[derive(Clone)]
pub struct CsrfToken(String);

/// Define custom methods and functions for the `CsrfToken` type itself.
/// Again, it is like defining methods in a blueprint or abstract class.
impl CsrfToken {
    /// Generates an authenticity token using the stored CSRF token.
    ///
    /// This function generates an authenticity token based on the stored CSRF token. The authenticity
    /// token is typically used in forms and requests to prevent Cross-Site Request Forgery attacks.
    /// It provides an additional layer of security to ensure that the request is legitimate.
    ///
    /// # Returns
    /// (`Result<String, BcryptError>`): The generated authenticity token or an error if token generation fails.
    pub fn authenticity_token(&self) -> Result<String, BcryptError> {
        // Handle potential errors from the hash function.
        match hash(&self.0, BCRYPT_COST) {
            Ok(token) => Ok(token),
            Err(err) => Err(err),
        }
    }

    /// Verifies if a provided token matches the stored CSRF token.
    /// # Arguments
    /// * `form_authenticity_token` - The token to verify.
    ///
    /// This function verifies if the provided token matches the stored CSRF token. It is commonly
    /// used to validate the authenticity of incoming requests. If the provided token matches the
    /// stored CSRF token, this function returns `Ok(())`. Otherwise, it returns an error of type `VerificationFailure`.
    ///
    /// # Returns
    /// (`Result<(), VerificationFailure>`): A result indicating success if the tokens match, or a `VerificationFailure`
    /// error if they do not.
    pub fn verify(&self, form_authenticity_token: &String) -> Result<(), VerificationFailure> {
        // Use a Result to propagate potential errors from the verify function.
        if verify(&self.0, form_authenticity_token).unwrap_or(false) {
            // CSRF token verification succeeded.
            info!("CSRF token verification succeeded.");
            Ok(())
        } else {
            Err(VerificationFailure {})
        }
    }
}

#[async_trait]
impl RocketFairing for Fairing {
    /// Get information about the CSRF protection fairing, including its name and kind.
    ///
    /// # Returns
    /// (`Info`): Information about the CSRF protection fairing.
    fn info(&self) -> Info {
        Info {
            name: "CSRF",
            kind: Kind::Ignite | Kind::Request,
        }
    }

    /// Initialize the CSRF protection fairing when the Rocket application is ignited.
    /// # Arguments
    /// * `rocket` - The Rocket instance to initialize the fairing with.
    ///
    /// This function is responsible for initializing the CSRF protection fairing when the Rocket
    /// application is started. It ensures that the CSRF protection configuration is available for
    /// use in the application.
    ///
    /// # Returns
    /// (`Result<(), fairing::Error>`): A result indicating success or an error.
    async fn on_ignite(&self, rocket: Rocket<rocket::Build>) -> fairing::Result {
        Ok(rocket.manage(self.config.clone()))
    }

    /// Handle incoming requests and add CSRF cookies when necessary.
    /// # Arguments
    /// * `request` - The incoming request to handle.
    /// * `_data` - Data associated with the request.
    ///
    /// This function is responsible for handling incoming requests and adding CSRF cookies when necessary.
    /// It ensures that a valid CSRF token is available for each request to prevent Cross-Site Request Forgery attacks.
    ///
    /// # Examples
    /// ```
    /// // Handling incoming requests and adding CSRF cookies
    /// ```
    async fn on_request(&self, request: &mut Request<'_>, data: &mut Data<'_>) {
        let config = match request.guard::<&State<CsrfConfig>>().await {
            Outcome::Success(cfg) => cfg,
            Outcome::Error(e) => {
                // Log an error for the missing CSRF config.
                error!("CSRF config is missing: {:?}", e);
                return;
            }
            Outcome::Forward(_) => {
                // Log an error for the forward case.
                error!("Request should be forwarded");
                return;
            }
        };

        if let Some(_) = request.valid_csrf_token_from_session(&config) {
            return;
        }

        let values: Vec<u8> = rand::rng()
            .sample_iter(StandardUniform)
            .take(config.cookie_len)
            .collect();

        let encoded = general_purpose::STANDARD.encode(&values[..]);

        let expires = match config.lifespan {
            Some(duration) => Some(OffsetDateTime::now_utc() + duration),
            None => None, // Expiration of None means a session cookie
        };

        let cookie_builder = Cookie::build((config.cookie_name.clone(), encoded)).path("/");

        let cookie_builder = match expires {
            Some(expiration) => cookie_builder.expires(expiration),
            None => cookie_builder.expires(None), // Expiration of None means duration of session
                                                  // Reference: https://api.rocket.rs/master/rocket/http/struct.Cookie.html#method.set_expires
        };

        let cookie = cookie_builder.build();

        if request.cookies().add_private(cookie) == () {
            // The cookie was added successfully.
            info!("CSRF cookie added successfully.");
        } else {
            // Handle the case where adding the CSRF cookie fails.
            // Log an error.
            error!("Failed to add CSRF cookie");
        }
        let _ = CsrfToken("".to_string()).on_request(request, data).await;
    }
}

#[async_trait]
impl<'r> FromRequest<'r> for CsrfToken {
    type Error = ();

    /// Create a CsrfToken from the request or return a Forbidden status if it's not valid.
    /// # Arguments
    /// * `request` - The request from which to extract the token.
    ///
    /// This function is responsible for creating a CsrfToken from the request or returning a Forbidden
    /// status if the token is not valid. It ensures that the CsrfToken is available for use in the application.
    ///
    /// # Returns
    /// (`Outcome<Self, Self::Error>`): An outcome indicating success with a CsrfToken or a Forbidden status on failure.
    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let config = request.guard::<&State<CsrfConfig>>().await.unwrap();

        match request.valid_csrf_token_from_session(&config) {
            Some(token) => {
                let encoded = general_purpose::STANDARD.encode(token);
                Outcome::Success(Self(encoded))
            }
            None => Outcome::Error((Status::Forbidden, ())),
        }
    }
}

impl fmt::Display for CsrfToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// TODO

fn _ajax_csrf_meta_tags(request: &Request) -> String {
    // Retrieve the CSRF token from the request headers
    let csrf_token = request.local_cache(|| CsrfToken("".to_string())); // Modify this to get the actual token

    // Generate the HTML meta tags
    format!(
        r#"<meta name="csrf-token" content="{}">
           <meta name="csrf-param" content="{}">"#,
        csrf_token, _PARAM_NAME
    )
}

struct _AjaxCsrfMetaTagsResponder<'o>(Response<'o>);

// impl<'r> Responder<'r, 'static> for AjaxCsrfMetaTagsResponder<'_> {
//     fn respond_to(self, request: &Request) -> rocket::response::Result<'static> {
//         let csrf_meta_tags = ajax_csrf_meta_tags(request);
//         let body = format!(
//             "<!DOCTYPE html>\n<html>\n<head>{}</head>\n<body></body>\n</html>",
//             csrf_meta_tags
//         );

//         Response::build()
//             .header(ContentType::HTML)
//             .sized_body(Cursor::new(body))
//             .respond_to(request)
//     }
// }

#[async_trait]
impl RocketFairing for CsrfToken {
    /// Provide information about the fairing.
    fn info(&self) -> Info {
        Info {
            name: "VerifyAllRequests",
            kind: Kind::Request,
        }
    }

    /// Perform CSRF token verification on incoming requests.
    ///
    /// This function is called on every incoming request, where it verifies the authenticity of the
    /// request by checking the CSRF token in the request headers. It handles cases where the CSRF
    /// token is missing, invalid, or requires forwarding.
    ///
    /// # Arguments
    /// * `request` - A mutable reference to the incoming request.
    /// * `_data` - A mutable reference to the Rocket Data.
    async fn on_request(&self, request: &mut Request<'_>, _data: &mut Data<'_>) {
        // Retrieve CSRF token from the request and CSRF configuration
        let csrf_token = request.headers().get_one(HEADER_NAME).map(String::from);
        let csrf_config = request.guard::<&State<CsrfConfig>>().await;
        match csrf_config {
            Outcome::Success(_config) => {
                // CSRF config is available, continue with verification
                if csrf_token.is_some() {
                    match self.verify(&csrf_token.clone().unwrap()) {
                        Ok(_) => {
                            // Request is valid, continue processing
                            // CsrfToken is successfully created, add it to the request's local cache
                            info!("CsrfToken is successfully created");
                            request.local_cache(|| CsrfToken(csrf_token.unwrap()));
                        }
                        Err(err) => {
                            // Handle the VerificationFailure error
                            // Log the error
                            error!("{:?}", err);
                            // TODO: Set the response status to Forbidden
                            // return an error response to the client
                        }
                    }
                } else {
                    // Handle the case where the request lacks an authenticity token
                    // Log the error or perform appropriate error handling
                    error!("Request lacks X-CSRF-Token");

                    // TODO: Set the response status to Forbidden
                    // return an error response to the client
                }
            }
            Outcome::Error(e) => {
                // Handle the case where CSRF config is missing
                // Log the error or perform appropriate error handling
                error!("CSRF config is missing: {:?}", e);

                // TODO: Set the response status to Forbidden
                // return an error response to the client
            }
            Outcome::Forward(_) => {
                // Handle the case where the request should be forwarded
                // Log the error or perform appropriate error handling
                error!("Request should be forwarded");
            }
        }
    }

    async fn on_response<'r>(&self, _req: &'r Request<'_>, res: &mut Response<'r>) {
        // Check if the response is HTML
        if let Some(content_type) = res.content_type() {
            if content_type.is_html() {
                // TODO:
                // res.set_body(AjaxCsrfMetaTagsResponder(res.take()));
            }
        }
    }
}

/// Custom error type for CSRF token verification failure. It is returned when CSRF token
/// verification fails during request processing.
pub struct VerificationFailure;

impl fmt::Debug for VerificationFailure {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "CSRF token verification failed!")
    }
}

// Implement Responder for VerificationFailure to return a Forbidden status response
impl<'r> Responder<'r, 'static> for VerificationFailure {
    fn respond_to(self, _request: &Request) -> rocket::response::Result<'static> {
        // Create a Forbidden response
        let response = Response::build().status(Status::Forbidden).finalize();

        Ok(response)
    }
}

/// Trait for CSRF-related request functions.
trait RequestCsrf {
    /// Check if a valid CSRF token exists in the session and has a sufficient length.
    /// # Arguments
    /// * `config` - The CsrfConfig to use for checking the CSRF token.
    ///
    /// This function is responsible for checking if a valid CSRF token exists in the session and has
    /// a sufficient length to be considered valid.
    ///
    /// # Returns
    /// (`Option<Vec<u8>>`): Some if the token is valid, None otherwise.
    fn valid_csrf_token_from_session(&self, config: &CsrfConfig) -> Option<Vec<u8>> {
        match self.csrf_token_from_session(config) {
            Some(raw) if raw.len() >= config.cookie_len => Some(raw),
            _ => None,
        }
    }

    /// Retrieve the CSRF token from the session and decode it.
    /// # Arguments
    /// * `config` - The CsrfConfig to use for retrieving the CSRF token.
    ///
    /// This function is responsible for retrieving the CSRF token from the session and decoding it
    /// to make it usable for token verification and authenticity token generation.
    ///
    /// # Returns
    /// (`Option<Vec<u8>>`): Some with the decoded token if found, None otherwise.
    fn csrf_token_from_session(&self, config: &CsrfConfig) -> Option<Vec<u8>>;
}

impl RequestCsrf for Request<'_> {
    /// Retrieve and decode the CSRF token from the session.
    ///
    /// This function retrieves and decodes the CSRF token from the session. It ensures that the token
    /// is available for use in the application, and that it can be verified and used to generate authenticity tokens.
    fn csrf_token_from_session(&self, config: &CsrfConfig) -> Option<Vec<u8>> {
        if let Some(cookie) = self.cookies().get_private(&config.cookie_name) {
            if let Ok(decoded) = general_purpose::STANDARD.decode(cookie.value()) {
                return Some(decoded);
            }
        }
        None
    }
}
