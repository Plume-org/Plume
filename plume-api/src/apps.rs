#[derive(Clone, Serialize, Deserialize)]
pub struct NewAppData {
    pub name: String,
    pub website: Option<String>,
    pub redirect_uri: Option<String>,
}
