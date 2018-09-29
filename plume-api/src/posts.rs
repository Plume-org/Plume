use canapi::Endpoint;

#[derive(Default, Serialize, Deserialize)]
pub struct PostEndpoint {
    pub id: Option<i32>,
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub content: Option<String>
}

api!("/api/v1/posts" => PostEndpoint);
