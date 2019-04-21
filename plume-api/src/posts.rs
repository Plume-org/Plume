#[derive(Clone, Default, Serialize, Deserialize)]
pub struct NewPostData {
    pub title: String,
    pub subtitle: Option<String>,
    pub source: String,
    pub author: String,
    // If None, a blog for this author will be choosen automatically
    pub blog_id: Option<i32>,
    pub published: Option<bool>,
    pub creation_date: Option<String>,
    pub license: Option<String>,
    pub tags: Option<Vec<String>>,
    pub cover_id: Option<i32>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct PostData {
    pub id: i32,
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub source: Option<String>,
    pub authors: Vec<String>,
    pub blog_id: i32,
    pub published: bool,
    pub creation_date: String,
    pub license: String,
    pub tags: Vec<String>,
    pub cover_id: Option<i32>,
}
