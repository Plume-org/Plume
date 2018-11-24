use canapi::{Error as ApiError, Provider};
use rocket::http::uri::Origin;
use rocket_contrib::json::Json;
use scheduled_thread_pool::ScheduledThreadPool;
use serde_json;
use serde_qs;

use plume_api::posts::PostEndpoint;
use plume_models::{
    Connection,
    db_conn::DbConn,
    posts::Post,
    search::Searcher as UnmanagedSearcher,
};
use api::authorization::*;
use {Searcher, Worker};

#[get("/posts/<id>")]
pub fn get(id: i32, conn: DbConn, worker: Worker, auth: Option<Authorization<Read, Post>>, search: Searcher) -> Json<serde_json::Value> {
    let post = <Post as Provider<(&Connection, &ScheduledThreadPool, &UnmanagedSearcher, Option<i32>)>>
        ::get(&(&*conn, &worker, &search, auth.map(|a| a.0.user_id)), id).ok();
    Json(json!(post))
}

#[get("/posts")]
pub fn list(conn: DbConn, uri: &Origin, worker: Worker, auth: Option<Authorization<Read, Post>>, search: Searcher) -> Json<serde_json::Value> {
    let query: PostEndpoint = serde_qs::from_str(uri.query().unwrap_or("")).expect("api::list: invalid query error");
    let post = <Post as Provider<(&Connection, &ScheduledThreadPool, &UnmanagedSearcher, Option<i32>)>>
        ::list(&(&*conn, &worker, &search, auth.map(|a| a.0.user_id)), query);
    Json(json!(post))
}

#[post("/posts", data = "<payload>")]
pub fn create(conn: DbConn, payload: Json<PostEndpoint>, worker: Worker, auth: Authorization<Write, Post>, search: Searcher) -> Json<serde_json::Value> {
    let new_post = <Post as Provider<(&Connection, &ScheduledThreadPool, &UnmanagedSearcher, Option<i32>)>>
        ::create(&(&*conn, &worker, &search, Some(auth.0.user_id)), (*payload).clone());
    Json(new_post.map(|p| json!(p)).unwrap_or_else(|e| json!({
        "error": "Invalid data, couldn't create new post",
        "details": match e {
            ApiError::Fetch(msg) => msg,
            ApiError::SerDe(msg) => msg,
            ApiError::NotFound(msg) => msg,
            ApiError::Authorization(msg) => msg,
        }
    })))
}

#[cfg(test)]
mod tests {
    use diesel;
    use plume_common::utils::random_hex;
    use plume_models::{
        Connection as Conn,
        api_tokens::*,
        apps::*,
        blogs::*,
        blog_authors::*,
        db_conn::{DbConn, DbPool},
        instance::*,
        posts::Post,
        safe_string::SafeString,
        schema,
        users::*,
    };
    use rocket::http::{Header, ContentType};
    use serde_json;
    use test_client;

    fn setup_db(conn: &Conn) {
        diesel::delete(schema::instances::table);
        diesel::delete(schema::blogs::table);
        diesel::delete(schema::blog_authors::table);
        diesel::delete(schema::users::table);
        diesel::delete(schema::posts::table);

        Instance::insert(conn, NewInstance {
            default_license: "WTFPL".to_string(),
            local: true,
            long_description: SafeString::new("This is my instance."),
            long_description_html: "<p>This is my instance</p>".to_string(),
            short_description: SafeString::new("My instance."),
            short_description_html: "<p>My instance</p>".to_string(),
            name: "My instance".to_string(),
            open_registrations: true,
            public_domain: "plu.me".to_string(),
        });
        let user = NewUser::new_local(
            conn,
            "admin".to_owned(),
            "The admin".to_owned(),
            true,
            "Hello there, I'm the admin".to_owned(),
            "admin@example.com".to_owned(),
            "invalid_admin_password".to_owned(),
        );
        let blog = Blog::insert(conn, NewBlog::new_local(
            "MyBlog".to_owned(),
            "My blog".to_owned(),
            "Welcome to my blog".to_owned(),
            Instance::local_id(conn),
        ));
        BlogAuthor::insert(conn, NewBlogAuthor {
            blog_id: blog.id,
            author_id: user.id,
            is_owner: true,
        });
    }

    fn api_token_for(conn: &Conn, user: User) -> ApiToken {
        let client_id = random_hex();
        let client_secret = random_hex();
        let app = App::insert(conn, NewApp {
            name: "Test app".to_string(),
            client_id: client_id,
            client_secret: client_secret,
            redirect_uri: None,
            website: None,
        });

        ApiToken::insert(conn, NewApiToken {
            app_id: app.id,
            user_id: user.id,
            value: random_hex(),
            scopes: "write".to_string(),
        })
    }

    #[test]
    fn create() {
        let client = test_client();
        let pool = client.rocket().state::<DbPool>().expect("DbPool is not managed");
        let conn = DbConn(pool.get().expect("Test DB pool error"));
        setup_db(&*conn);
        let auth = api_token_for(&*conn, User::get(&*conn, 1).unwrap());

        let title = format!("My new post {}", random_hex()); // random string to avoid slug collisions
        let mut res = client.post("/api/v1/posts")
            .header(ContentType::JSON)
            .header(Header::new("Authorization", format!("Bearer {}", auth.value)))
            .body(json!({
                "title": title,
                "source": "**Markdown**",
                "tags": [ "Hello", "World" ],
            }).to_string())
            .dispatch();
        let json: serde_json::Value = serde_json::from_str(res.body_string().expect("Body as string error").as_str()).unwrap();
        println!("{:?}", json);
        let post = Post::get(&*conn, json["id"].as_i64().unwrap() as i32).unwrap();
        assert_eq!(post.title, title);
        assert_eq!(post.content.get(), "<p><strong>Markdown</strong></p>\n");
    }
}
