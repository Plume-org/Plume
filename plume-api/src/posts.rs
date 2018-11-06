use canapi::Endpoint;

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct PostEndpoint {
    pub id: Option<i32>,
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub content: Option<String>,
    pub source: Option<String>,
    pub author: Option<String>,
    pub blog_id: Option<i32>,
    pub published: Option<bool>,
    pub creation_date: Option<String>,
    pub license: Option<String>,
    pub tags: Option<Vec<String>>,
    pub cover_id: Option<i32>,
}

api!("/api/v1/posts" => PostEndpoint);
