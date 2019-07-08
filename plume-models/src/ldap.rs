use crate::CONFIG;
use ldap3::LdapConn;
use std::io;
use std::sync::{mpsc, Mutex};
use std::thread;

type Message = (String, String, mpsc::Sender<io::Result<bool>>);
pub struct Ldap {
    channel: mpsc::Sender<Message>,
}

impl Ldap {
    pub fn get_shared() -> Self {
        Ldap {
            channel: CHANNEL.lock().unwrap().clone(),
        }
    }

    pub fn connect(&self, username: String, password: String) -> LdapResult {
        let (s, r) = mpsc::channel();
        self.channel.send((username, password, s)).unwrap(); //we know the remote end was not closed
        LdapResult { channel: r }
    }
}

pub struct LdapResult {
    channel: mpsc::Receiver<io::Result<bool>>,
}

impl LdapResult {
    pub fn get(self) -> io::Result<bool> {
        self.channel.recv().unwrap() //we know some message must have been send, be it an error
    }
}

/// This function loop indefinitelly, handling requests
fn handle(url: &str, bind_dn: &str, channel: mpsc::Receiver<Message>) {
    let mut conn = LdapConn::new(url).expect("Error connecting to ldap server");
    for (user, password, channel) in channel.iter() {
        let res = conn
            .simple_bind(&format!("uid={},{}", user, bind_dn), &password)
            .map(|r| r.rc == 0);
        let err = res.is_err();
        channel.send(res).ok(); //we can't assume the other end did not drop it's handle
        let err = conn.unbind().is_err() || err;
        if err {
            if let Ok(c) = LdapConn::new(url) {
                conn = c;
            }
        }
    }
}

fn ignore(channel: mpsc::Receiver<Message>) {
    for (_user, _password, channel) in channel.iter() {
        channel.send(Ok(false)).ok();
    }
}

lazy_static! {
    static ref CHANNEL: Mutex<mpsc::Sender<Message>> = {
        let (s, r) = mpsc::channel();

        let builder = thread::Builder::new().name("ldap_handler".into());
        builder
            .spawn(move || {
                if CONFIG.ldap.url.is_some() && CONFIG.ldap.bind_dn.is_some() {
                    handle(
                        CONFIG.ldap.url.as_ref().unwrap(),
                        CONFIG.ldap.bind_dn.as_ref().unwrap(),
                        r,
                    )
                } else {
                    ignore(r);
                }
            })
            .unwrap();
        Mutex::new(s)
    };
}
