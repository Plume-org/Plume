#[derive(Clone, Default, Serialize, Deserialize)]
pub struct TranslationRequest {
    pub target_lang: String,
    pub source_lang: Option<String>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct TranslationData {
    pub id: i32,
    pub post_id: i32,
    pub source_lang: String,
    pub target_lang: String,
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub source: String,
    pub creation_date: String,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct TranslatedPostData {
    pub post_id: i32,
    pub source_lang: String,
    pub target_lang: String,
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub source: String,
}
