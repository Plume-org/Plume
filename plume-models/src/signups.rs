use crate::CONFIG;
use rocket::{
    http::Status,
    request::{FromRequest, Outcome, Request},
};
use std::fmt;
use std::str::FromStr;

pub enum Strategy {
    Password,
    Email,
}

impl Default for Strategy {
    fn default() -> Self {
        Self::Password
    }
}

impl FromStr for Strategy {
    type Err = StrategyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use self::Strategy::*;

        match s {
            "password" => Ok(Password),
            "email" => Ok(Email),
            s => Err(StrategyError::Unsupported(s.to_string())),
        }
    }
}

#[derive(Debug)]
pub enum StrategyError {
    Unsupported(String),
}

impl fmt::Display for StrategyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use self::StrategyError::*;

        match self {
            // FIXME: Calc option strings from enum
            Unsupported(s) => write!(f, "Unsupported strategy: {}. Choose password or email", s),
        }
    }
}

impl std::error::Error for StrategyError {}

pub struct Password();
pub struct Email();

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Password {
    type Error = ();

    async fn from_request(_request: &'r Request<'_>) -> Outcome<Password, Self::Error> {
        match matches!(CONFIG.signup, Strategy::Password) {
            true => Outcome::Success(Self()),
            false => Outcome::Forward(Status::Unauthorized),
        }
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Email {
    type Error = ();

    async fn from_request(_request: &'r Request<'_>) -> Outcome<Email, Self::Error> {
        match matches!(CONFIG.signup, Strategy::Email) {
            true => Outcome::Success(Self()),
            false => Outcome::Forward(Status::Unauthorized),
        }
    }
}
