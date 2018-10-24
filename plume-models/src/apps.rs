use canapi::{Error, Provider};
use chrono::NaiveDateTime;
use diesel::{self, RunQueryDsl, QueryDsl, ExpressionMethods};

use plume_api::apps::AppEndpoint;
use plume_common::utils::random_hex;
use Connection;
use schema::apps;

#[derive(Clone, Queryable)]
pub struct App {
    pub id: i32,
    pub name: String,
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: Option<String>,
    pub website: Option<String>,
    pub creation_date: NaiveDateTime,
}

#[derive(Insertable)]
#[table_name= "apps"]
pub struct NewApp {
    pub name: String,
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: Option<String>,
    pub website: Option<String>,
}

impl Provider<Connection> for App {
    type Data = AppEndpoint;

    fn get(_conn: &Connection, _id: i32) -> Result<AppEndpoint, Error> {
        unimplemented!()
    }

    fn list(_conn: &Connection, _query: AppEndpoint) -> Vec<AppEndpoint> {
        unimplemented!()
    }

    fn create(conn: &Connection, data: AppEndpoint) -> Result<AppEndpoint, Error> {
        let client_id = random_hex();

        let client_secret = random_hex();
        let app = App::insert(conn, NewApp {
            name: data.name,
            client_id: client_id,
            client_secret: client_secret,
            redirect_uri: data.redirect_uri,
            website: data.website,
        });

        Ok(AppEndpoint {
            id: Some(app.id),
            name: app.name,
            client_id: Some(app.client_id),
            client_secret: Some(app.client_secret),
            redirect_uri: app.redirect_uri,
            website: app.website,
        })
    }

    fn update(_conn: &Connection, _id: i32, _new_data: AppEndpoint) -> Result<AppEndpoint, Error> {
        unimplemented!()
    }

    fn delete(_conn: &Connection, _id: i32) {
        unimplemented!()
    }
}

impl App {
    get!(apps);
    insert!(apps, NewApp);
    find_by!(apps, find_by_client_id, client_id as String);
}
