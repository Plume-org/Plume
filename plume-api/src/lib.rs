extern crate canapi;
extern crate serde;
#[macro_use]
extern crate serde_derive;

macro_rules! api {
    ($url:expr => $ep:ty) => {
        impl Endpoint for $ep {
            type Id = i32;

            fn endpoint() -> &'static str {
                $url
            }
        }
    };
}

pub mod posts;
