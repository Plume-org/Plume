use canapi::Endpoint;

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct AppEndpoint {
    pub id: Option<i32>,
    pub name: String,
    pub website: Option<String>,
    pub redirect_uri: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
}

api!("/api/v1/apps" => AppEndpoint);
