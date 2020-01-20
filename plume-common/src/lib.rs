#![feature(associated_type_defaults)]

#[macro_use]
extern crate activitystreams_derive;
use activitystreams_traits;

use serde;
#[macro_use]
extern crate shrinkwraprs;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;

pub mod activity_pub;
pub mod utils;
