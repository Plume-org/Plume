use lettre_email::Email;
use std::env;

pub use self::mailer::*;

#[cfg(feature = "debug-mailer")]
mod mailer {
    use lettre::{Transport, SendableEmail};
    use std::{io::Read};

    pub struct DebugTransport;

    impl<'a, T: Into<&'a [u8]> + Read + 'a> Transport<'a, T, Result<(), ()>> for DebugTransport {
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

    pub type Mailer = Option<DebugTransport>;

    pub fn init() -> Mailer {
        Some(DebugTransport)
    }
}

#[cfg(not(feature = "debug-mailer"))]
mod mailer {
    use lettre::{
        SmtpTransport,
        SmtpClient,
        smtp::{
            authentication::{Credentials, Mechanism},
            extension::ClientId,
            ConnectionReuseParameters,
        },
    };
    use std::env;

    pub type Mailer = Option<SmtpTransport>;

    pub fn init() -> Mailer {
        let server = env::var("MAIL_SERVER").ok()?;
        let helo_name = env::var("MAIL_HELO_NAME").unwrap_or_else(|_| "localhost".to_owned());
        let username = env::var("MAIL_USER").ok()?;
        let password = env::var("MAIL_PASSWORD").ok()?;
        let mail = SmtpClient::new_simple(&server).unwrap()
            .hello_name(ClientId::Domain(helo_name))
            .credentials(Credentials::new(username, password))
            .smtp_utf8(true)
            .authentication_mechanism(Mechanism::Plain)
            .connection_reuse(ConnectionReuseParameters::NoReuse)
            .transport();
        Some(mail)
    }
}

pub fn build_mail(dest: String, subject: String, body: String) -> Option<Email> {
    Email::builder()
        .from(env::var("MAIL_ADDRESS")
            .or_else(|_| Ok(format!("{}@{}", env::var("MAIL_USER")?, env::var("MAIL_SERVER")?)) as Result<_, env::VarError>)
            .expect("Mail server is not correctly configured"))
        .to(dest)
        .subject(subject)
        .text(body)
        .build()
        .ok()
}
