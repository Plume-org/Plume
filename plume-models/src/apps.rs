use chrono::NaiveDateTime;

use schema::apps;

pub struct App {
    pub id: i32,
    pub name: String,
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: Option<String>,
    pub website: Option<String>,    
    pub creation_date: NaiveDateTime,
}

impl App {
    get!(apps, App);
    insert!(apps, NewApp);
} 
