#![warn(clippy::too_many_arguments)]
use lettre_email::Email;
use std::env;

pub use self::mailer::*;

#[cfg(feature = "debug-mailer")]
mod mailer {
    use plume_models::smtp::{SendableEmail, Transport};
    use std::io::Read;

    pub struct DebugTransport;

    impl<'a> Transport<'a> for DebugTransport {
        type Result = Result<(), ()>;

        fn send(&mut self, email: SendableEmail) -> Self::Result {
            println!(
                "{}: from=<{}> to=<{:?}>\n{:#?}",
                email.message_id().to_string(),
                email
                    .envelope()
                    .from()
                    .map(ToString::to_string)
                    .unwrap_or_default(),
                email.envelope().to().to_vec(),
                {
                    let mut message = String::new();
                    email
                        .message()
                        .read_to_string(&mut message)
                        .map_err(|_| ())?;
                    message
                },
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
    use plume_models::smtp::{
        authentication::{Credentials, Mechanism},
        extension::ClientId,
        ConnectionReuseParameters, SmtpClient, SmtpTransport,
    };
    use plume_models::{SmtpNewWithAddr, CONFIG};

    pub type Mailer = Option<SmtpTransport>;

    pub fn init() -> Mailer {
        let config = CONFIG.mail.as_ref()?;
        let mail = SmtpClient::new_with_addr((&config.server, config.port))
            .unwrap()
            .hello_name(ClientId::Domain(config.helo_name.clone()))
            .credentials(Credentials::new(
                config.username.clone(),
                config.password.clone(),
            ))
            .smtp_utf8(true)
            .authentication_mechanism(Mechanism::Plain)
            .connection_reuse(ConnectionReuseParameters::NoReuse)
            .transport();
        Some(mail)
    }
}

pub fn build_mail(dest: String, subject: String, body: String) -> Option<Email> {
    Email::builder()
        .from(
            env::var("MAIL_ADDRESS")
                .or_else(|_| {
                    Ok(format!(
                        "{}@{}",
                        env::var("MAIL_USER")?,
                        env::var("MAIL_SERVER")?
                    )) as Result<_, env::VarError>
                })
                .expect("The email server is not configured correctly"),
        )
        .to(dest)
        .subject(subject)
        .text(body)
        .build()
        .ok()
}
