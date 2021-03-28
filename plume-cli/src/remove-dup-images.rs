use diesel::{
    BoolExpressionMethods, Connection, ExpressionMethods, JoinOnDsl, NullableExpressionMethods,
    QueryDsl, RunQueryDsl,
};
use plume_models::{
    blogs::Blog, instance::Instance, medias::Media, posts::Post, Connection as Conn, CONFIG,
};
use std::collections::hash_map::{DefaultHasher, HashMap};
use std::fs::File;
use std::hash::Hasher;
use std::io::{BufReader, Read};
use std::path::Path;

fn main() {
    match dotenv::dotenv() {
        Ok(path) => eprintln!("Configuration read from {}", path.display()),
        Err(ref e) if e.not_found() => eprintln!("no .env was found"),
        e => e.map(|_| ()).unwrap(),
    }
    let conn = Conn::establish(CONFIG.database_url.as_str()).expect("extablish connection");
    Instance::cache_local(&conn);
    let covers = get_remote_post_covers(&conn);
    let remote_media_hashes = calculate_remote_media_hashes(covers);
    eprintln!("remote medias: {:?}", remote_media_hashes);
    let orphan_medias = get_orphan_medias(&conn);
    eprintln!("{:?} orphan media(s)", orphan_medias.len());
    for media in orphan_medias {
        match calculate_file_hash(&Path::new(&media.file_path)) {
            Some(hash) => {
                match remote_media_hashes.get(&hash) {
                    Some(file_path) => {
                        eprintln!(
                            "File already referred. Removes only medias record. {}",
                            &file_path
                        );
                        // Remove medias record
                        diesel::delete(&media)
                            .execute(&conn)
                            .expect("Delete medias record");
                    }
                    None => {
                        eprintln!("Removes {}", &media.file_path);
                        // Remove file and medias record
                        media.delete(&conn).expect("Delete media record and file");
                    }
                }
            }
            None => {
                eprintln!(
                    "File doesn't exist. Removes medias record. medias.id: {}, path: {}",
                    &media.id, &media.file_path
                );
                diesel::delete(&media)
                    .execute(&conn)
                    .expect("Delete medias record");
            }
        }
    }
}

fn get_remote_post_covers(conn: &Conn) -> Vec<Media> {
    use plume_models::schema::blogs;
    use plume_models::schema::posts;

    let remote_instances = Instance::get_remotes(&conn).expect("get remote instances");
    let remote_instance_ids = remote_instances.iter().map(|instance| instance.id);
    let remote_blogs = blogs::table
        .filter(blogs::instance_id.eq_any(remote_instance_ids))
        .load::<Blog>(conn)
        .expect("remote blogs");
    let remote_blog_ids = remote_blogs.iter().map(|blog| blog.id);
    let remote_posts = posts::table
        .filter(posts::blog_id.eq_any(remote_blog_ids))
        .load::<Post>(conn)
        .expect("remote posts");
    remote_posts
        .iter()
        .filter_map(|post| post.cover_id)
        .map(|cover_id| Media::get(conn, cover_id).expect("Media"))
        .collect()
}

fn calculate_remote_media_hashes(medias: Vec<Media>) -> HashMap<u64, String> {
    let mut media_hashes = HashMap::new();
    for media in medias.iter() {
        if let Some(hash) = calculate_file_hash(Path::new(&media.file_path)) {
            let _ = media_hashes.insert(hash, media.file_path.clone());
        }
    }
    media_hashes
}

fn calculate_file_hash(path: &Path) -> Option<u64> {
    if !path.exists() {
        return None;
    }
    let file = File::open(path).expect("open file");
    let mut reader = BufReader::new(file);
    let mut hasher = DefaultHasher::new();
    let mut buffer = [0; 2048];

    while let Ok(n) = reader.read(&mut buffer) {
        hasher.write(&buffer);

        if n == 0 {
            break;
        }
    }
    Some(hasher.finish())
}

fn get_orphan_medias(conn: &Conn) -> Vec<Media> {
    use plume_models::schema::{self, medias};
    use plume_models::schema::{blogs::dsl::blogs, posts::dsl::posts, users::dsl::users};
    let query = medias::table
        .select((
            medias::id,
            medias::file_path,
            medias::alt_text,
            medias::is_remote,
            medias::remote_url,
            medias::sensitive,
            medias::content_warning,
            medias::owner_id,
        ))
        .left_outer_join(users.on(schema::users::avatar_id.eq(medias::id.nullable())))
        .left_outer_join(
            blogs.on(schema::blogs::icon_id
                .eq(medias::id.nullable())
                .or(schema::blogs::banner_id.eq(medias::id.nullable()))),
        )
        .left_outer_join(posts.on(schema::posts::cover_id.eq(medias::id.nullable())))
        .filter(
            schema::users::avatar_id.is_null().and(
                schema::blogs::icon_id.is_null().and(
                    schema::blogs::banner_id.is_null().and(
                        schema::posts::cover_id
                            .is_null()
                            .and(medias::is_remote.eq(false)),
                    ),
                ),
            ),
        );
    eprintln!(
        "query for orphan medias: {}",
        diesel::debug_query::<_, _>(&query)
    );
    query.load::<Media>(conn).expect("Load orphan medias")
}
