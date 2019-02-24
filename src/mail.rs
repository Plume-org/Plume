#[cfg(not(feature = "debug-mailer"))]
use lettre::{
    SmtpTransport,
    smtp::{
        authentication::{Credentials, Mechanism},
        extension::ClientId,
        ConnectionReuseParameters,
    },
};
#[cfg(feature = "debug-mailer")]
use lettre::{EmailTransport, SendableEmail};
#[cfg(not(feature = "debug-mailer"))]
use std::env;
#[cfg(feature = "debug-mailer")]
use std::{io::Read};

#[cfg(feature = "debug-mailer")]
pub struct DebugTransport;

#[cfg(feature = "debug-mailer")]
impl<'a, T: Into<&'a [u8]> + Read + 'a> EmailTransport<'a, T, Result<(), ()>> for DebugTransport {
    fn send<U: SendableEmail<'a, T> + 'a>(&mut self, email: &'a U) -> Result<(), ()> {
        let message = *email.message();
        println!(
            "{}: from=<{}> to=<{:?}>\n{:#?}",
            email.message_id(),
            match email.envelope().from() {
                Some(address) => address.to_string(),
                None => "".to_string(),
            },
            email.envelope().to(),
            String::from_utf8(message.into().to_vec()),
        );
        Ok(())
    }
}

#[cfg(feature = "debug-mailer")]
pub type Mailer = Option<DebugTransport>;

#[cfg(feature = "debug-mailer")]
pub fn init() -> Mailer {
    Some(DebugTransport)
}

#[cfg(not(feature = "debug-mailer"))]
pub type Mailer = Option<SmtpTransport>;

#[cfg(not(feature = "debug-mailer"))]
pub fn init() -> Mailer {
    let server = env::var("MAIL_SERVER").ok()?;
    let helo_name = env::var("MAIL_HELO_NAME").unwrap_or_else(|_| "localhost".to_owned());
    let username = env::var("MAIL_USER").ok()?;
    let password = env::var("MAIL_PASSWORD").ok()?;
    let mail = SmtpTransport::simple_builder(&server).unwrap()
        .hello_name(ClientId::Domain(helo_name))
        .credentials(Credentials::new(username, password))
        .smtp_utf8(true)
        .authentication_mechanism(Mechanism::Plain)
        .connection_reuse(ConnectionReuseParameters::NoReuse).build();
    Some(mail)
}
