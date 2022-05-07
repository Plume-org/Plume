use activitystreams::{
    actor::{ApActor, Group, Person},
    base::{AnyBase, Base, Extends},
    iri_string::types::IriString,
    kind,
    markers::{self, Activity},
    object::{ApObject, Article, Object},
    primitives::{AnyString, OneOrMany},
    unparsed::UnparsedMutExt,
};
use activitystreams_ext::{Ext1, Ext2, UnparsedExtension};
use array_tool::vec::Uniq;
use futures::future::join_all;
use reqwest::{header::HeaderValue, ClientBuilder, RequestBuilder, Url};
use rocket::{
    http::Status,
    request::{FromRequest, Request},
    response::{Responder, Response},
    Outcome,
};
use tokio::{
    runtime,
    time::{sleep, Duration},
};
use tracing::{debug, warn};

use self::sign::Signable;

pub mod inbox;
pub mod request;
pub mod sign;

pub const CONTEXT_URL: &str = "https://www.w3.org/ns/activitystreams";
pub const PUBLIC_VISIBILITY: &str = "https://www.w3.org/ns/activitystreams#Public";

pub const AP_CONTENT_TYPE: &str =
    r#"application/ld+json; profile="https://www.w3.org/ns/activitystreams""#;

pub fn ap_accept_header() -> Vec<&'static str> {
    vec![
        "application/ld+json; profile=\"https://www.w3.org/ns/activitystreams\"",
        "application/ld+json;profile=\"https://www.w3.org/ns/activitystreams\"",
        "application/activity+json",
        "application/ld+json",
    ]
}

pub fn context() -> serde_json::Value {
    json!([
        CONTEXT_URL,
        "https://w3id.org/security/v1",
        {
            "manuallyApprovesFollowers": "as:manuallyApprovesFollowers",
            "sensitive": "as:sensitive",
            "movedTo": "as:movedTo",
            "Hashtag": "as:Hashtag",
            "ostatus":"http://ostatus.org#",
            "atomUri":"ostatus:atomUri",
            "inReplyToAtomUri":"ostatus:inReplyToAtomUri",
            "conversation":"ostatus:conversation",
            "toot":"http://joinmastodon.org/ns#",
            "Emoji":"toot:Emoji",
            "focalPoint": {
                "@container":"@list",
                "@id":"toot:focalPoint"
            },
            "featured":"toot:featured"
        }
    ])
}

pub struct ActivityStream<T>(T);

impl<T> ActivityStream<T> {
    pub fn new(t: T) -> ActivityStream<T> {
        ActivityStream(t)
    }
}

impl<'r, O: serde::Serialize> Responder<'r> for ActivityStream<O> {
    fn respond_to(self, request: &Request<'_>) -> Result<Response<'r>, Status> {
        let mut json = serde_json::to_value(&self.0).map_err(|_| Status::InternalServerError)?;
        json["@context"] = context();
        serde_json::to_string(&json).respond_to(request).map(|r| {
            Response::build_from(r)
                .raw_header("Content-Type", "application/activity+json")
                .finalize()
        })
    }
}

#[derive(Clone)]
pub struct ApRequest;
impl<'a, 'r> FromRequest<'a, 'r> for ApRequest {
    type Error = ();

    fn from_request(request: &'a Request<'r>) -> Outcome<Self, (Status, Self::Error), ()> {
        request
            .headers()
            .get_one("Accept")
            .map(|header| {
                header
                    .split(',')
                    .map(|ct| {
                        match ct.trim() {
                        // bool for Forward: true if found a valid Content-Type for Plume first (HTML), false otherwise
                        "application/ld+json; profile=\"https://www.w3.org/ns/activitystreams\""
                        | "application/ld+json;profile=\"https://www.w3.org/ns/activitystreams\""
                        | "application/activity+json"
                        | "application/ld+json" => Outcome::Success(ApRequest),
                        "text/html" => Outcome::Forward(true),
                        _ => Outcome::Forward(false),
                    }
                    })
                    .fold(Outcome::Forward(false), |out, ct| {
                        if out.clone().forwarded().unwrap_or_else(|| out.is_success()) {
                            out
                        } else {
                            ct
                        }
                    })
                    .map_forward(|_| ())
            })
            .unwrap_or(Outcome::Forward(()))
    }
}

pub fn broadcast<S, A, T, C>(sender: &S, act: A, to: Vec<T>, proxy: Option<reqwest::Proxy>)
where
    S: sign::Signer,
    A: Activity + serde::Serialize,
    T: inbox::AsActor<C>,
{
    let boxes = to
        .into_iter()
        .filter(|u| !u.is_local())
        .map(|u| {
            u.get_shared_inbox_url()
                .unwrap_or_else(|| u.get_inbox_url())
        })
        .collect::<Vec<String>>()
        .unique();

    let mut act = serde_json::to_value(act).expect("activity_pub::broadcast: serialization error");
    act["@context"] = context();
    let signed = act
        .sign(sender)
        .expect("activity_pub::broadcast: signature error");

    let client = if let Some(proxy) = proxy {
        ClientBuilder::new().proxy(proxy)
    } else {
        ClientBuilder::new()
    }
    .connect_timeout(std::time::Duration::from_secs(5))
    .build()
    .expect("Can't build client");
    let rt = runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Error while initializing tokio runtime for federation");
    rt.block_on(async {
        // TODO: should be determined dependent on database connections because
        // after broadcasting, target instance sends request to this instance,
        // and Plume accesses database at that time.
        let capacity = 6;
        let (tx, rx) = flume::bounded::<RequestBuilder>(capacity);
        let mut handles = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            let rx = rx.clone();
            let handle = rt.spawn(async move {
                while let Ok(request_builder) = rx.recv_async().await {
                    // After broadcasting, target instance sends request to this instance.
                    // Sleep here in order to reduce requests at once
                    sleep(Duration::from_millis(500)).await;
                    let _ = request_builder
                        .send()
                        .await
                        .map(move |r| {
                            if r.status().is_success() {
                                debug!("Successfully sent activity to inbox ({})", &r.url());
                            } else {
                                warn!("Error while sending to inbox ({:?})", &r)
                            }
                            debug!("Response: \"{:?}\"\n", r);
                        })
                        .map_err(|e| warn!("Error while sending to inbox ({:?})", e));
                }
            });
            handles.push(handle);
        }
        for inbox in boxes {
            let body = signed.to_string();
            let mut headers = request::headers();
            let url = Url::parse(&inbox);
            if url.is_err() {
                warn!("Inbox is invalid URL: {:?}", &inbox);
                continue;
            }
            let url = url.unwrap();
            if !url.has_host() {
                warn!("Inbox doesn't have host: {:?}", &inbox);
                continue;
            };
            let host_header_value = HeaderValue::from_str(url.host_str().expect("Unreachable"));
            if host_header_value.is_err() {
                warn!("Header value is invalid: {:?}", url.host_str());
                continue;
            }
            headers.insert("Host", host_header_value.unwrap());
            headers.insert("Digest", request::Digest::digest(&body));
            headers.insert(
                "Signature",
                request::signature(sender, &headers, ("post", url.path(), url.query()))
                    .expect("activity_pub::broadcast: request signature error"),
            );
            let request_builder = client.post(&inbox).headers(headers.clone()).body(body);
            let _ = tx.send_async(request_builder).await;
        }
        drop(tx);
        join_all(handles).await;
    });
}

#[derive(Shrinkwrap, Clone, Serialize, Deserialize)]
pub struct Id(String);

impl Id {
    pub fn new(id: impl ToString) -> Id {
        Id(id.to_string())
    }
}

impl AsRef<str> for Id {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

pub trait IntoId {
    fn into_id(self) -> Id;
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ApSignature {
    pub public_key: PublicKey,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PublicKey {
    pub id: IriString,
    pub owner: IriString,
    pub public_key_pem: String,
}

impl<U> UnparsedExtension<U> for ApSignature
where
    U: UnparsedMutExt,
{
    type Error = serde_json::Error;

    fn try_from_unparsed(unparsed_mut: &mut U) -> Result<Self, Self::Error> {
        Ok(ApSignature {
            public_key: unparsed_mut.remove("publicKey")?,
        })
    }

    fn try_into_unparsed(self, unparsed_mut: &mut U) -> Result<(), Self::Error> {
        unparsed_mut.insert("publicKey", self.public_key)?;
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SourceProperty {
    pub source: Source,
}

impl<U> UnparsedExtension<U> for SourceProperty
where
    U: UnparsedMutExt,
{
    type Error = serde_json::Error;

    fn try_from_unparsed(unparsed_mut: &mut U) -> Result<Self, Self::Error> {
        Ok(SourceProperty {
            source: unparsed_mut.remove("source")?,
        })
    }

    fn try_into_unparsed(self, unparsed_mut: &mut U) -> Result<(), Self::Error> {
        unparsed_mut.insert("source", self.source)?;
        Ok(())
    }
}

pub type CustomPerson = Ext1<ApActor<Person>, ApSignature>;
pub type CustomGroup = Ext2<ApActor<Group>, ApSignature, SourceProperty>;

kind!(HashtagType, Hashtag);

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct Hashtag {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub href: Option<IriString>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<AnyString>,

    #[serde(flatten)]
    inner: Object<HashtagType>,
}

impl Hashtag {
    pub fn new() -> Self {
        Self {
            href: None,
            name: None,
            inner: Object::new(),
        }
    }

    pub fn extending(mut inner: Object<HashtagType>) -> Result<Self, serde_json::Error> {
        let href = inner.remove("href")?;
        let name = inner.remove("name")?;

        Ok(Self { href, name, inner })
    }

    pub fn retracting(self) -> Result<Object<HashtagType>, serde_json::Error> {
        let Self {
            href,
            name,
            mut inner,
        } = self;

        inner.insert("href", href)?;
        inner.insert("name", name)?;
        Ok(inner)
    }
}

pub trait AsHashtag: markers::Object {
    fn hashtag_ref(&self) -> &Hashtag;

    fn hashtag_mut(&mut self) -> &mut Hashtag;
}

pub trait HashtagExt: AsHashtag {
    fn href(&self) -> Option<&IriString> {
        self.hashtag_ref().href.as_ref()
    }

    fn set_href<T>(&mut self, href: T) -> &mut Self
    where
        T: Into<IriString>,
    {
        self.hashtag_mut().href = Some(href.into());
        self
    }

    fn take_href(&mut self) -> Option<IriString> {
        self.hashtag_mut().href.take()
    }

    fn delete_href(&mut self) -> &mut Self {
        self.hashtag_mut().href = None;
        self
    }

    fn name(&self) -> Option<&AnyString> {
        self.hashtag_ref().name.as_ref()
    }

    fn set_name<T>(&mut self, name: T) -> &mut Self
    where
        T: Into<AnyString>,
    {
        self.hashtag_mut().name = Some(name.into());
        self
    }

    fn take_name(&mut self) -> Option<AnyString> {
        self.hashtag_mut().name.take()
    }

    fn delete_name(&mut self) -> &mut Self {
        self.hashtag_mut().name = None;
        self
    }
}

impl Default for Hashtag {
    fn default() -> Self {
        Self::new()
    }
}

impl AsHashtag for Hashtag {
    fn hashtag_ref(&self) -> &Self {
        self
    }

    fn hashtag_mut(&mut self) -> &mut Self {
        self
    }
}

impl Extends<HashtagType> for Hashtag {
    type Error = serde_json::Error;

    fn extends(base: Base<HashtagType>) -> Result<Self, Self::Error> {
        let inner = Object::extends(base)?;
        Self::extending(inner)
    }

    fn retracts(self) -> Result<Base<HashtagType>, Self::Error> {
        let inner = self.retracting()?;
        inner.retracts()
    }
}

impl markers::Base for Hashtag {}
impl markers::Object for Hashtag {}
impl<T> HashtagExt for T where T: AsHashtag {}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub media_type: String,

    pub content: String,
}

impl<U> UnparsedExtension<U> for Source
where
    U: UnparsedMutExt,
{
    type Error = serde_json::Error;

    fn try_from_unparsed(unparsed_mut: &mut U) -> Result<Self, Self::Error> {
        Ok(Source {
            content: unparsed_mut.remove("content")?,
            media_type: unparsed_mut.remove("mediaType")?,
        })
    }

    fn try_into_unparsed(self, unparsed_mut: &mut U) -> Result<(), Self::Error> {
        unparsed_mut.insert("content", self.content)?;
        unparsed_mut.insert("mediaType", self.media_type)?;
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Licensed {
    pub license: Option<String>,
}

impl<U> UnparsedExtension<U> for Licensed
where
    U: UnparsedMutExt,
{
    type Error = serde_json::Error;

    fn try_from_unparsed(unparsed_mut: &mut U) -> Result<Self, Self::Error> {
        Ok(Licensed {
            license: unparsed_mut.remove("license")?,
        })
    }

    fn try_into_unparsed(self, unparsed_mut: &mut U) -> Result<(), Self::Error> {
        unparsed_mut.insert("license", self.license)?;
        Ok(())
    }
}

pub type LicensedArticle = Ext1<ApObject<Article>, Licensed>;

pub trait ToAsString {
    fn to_as_string(&self) -> Option<String>;
}

impl ToAsString for OneOrMany<&AnyString> {
    fn to_as_string(&self) -> Option<String> {
        self.as_as_str().map(|s| s.to_string())
    }
}

trait AsAsStr {
    fn as_as_str(&self) -> Option<&str>;
}

impl AsAsStr for OneOrMany<&AnyString> {
    fn as_as_str(&self) -> Option<&str> {
        self.iter().next().map(|prop| prop.as_str())
    }
}

pub trait ToAsUri {
    fn to_as_uri(&self) -> Option<String>;
}

impl ToAsUri for OneOrMany<AnyBase> {
    fn to_as_uri(&self) -> Option<String> {
        self.iter()
            .next()
            .and_then(|prop| prop.as_xsd_any_uri().map(|uri| uri.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use activitystreams::{
        activity::{ActorAndObjectRef, Create},
        object::kind::ArticleType,
    };
    use assert_json_diff::assert_json_eq;
    use serde_json::{from_str, json, to_value};

    #[test]
    fn se_ap_signature() {
        let ap_signature = ApSignature {
            public_key: PublicKey {
                id: "https://example.com/pubkey".parse().unwrap(),
                owner: "https://example.com/owner".parse().unwrap(),
                public_key_pem: "pubKeyPem".into(),
            },
        };
        let expected = json!({
            "publicKey": {
                "id": "https://example.com/pubkey",
                "owner": "https://example.com/owner",
                "publicKeyPem": "pubKeyPem"
            }
        });
        assert_json_eq!(to_value(ap_signature).unwrap(), expected);
    }

    #[test]
    fn de_ap_signature() {
        let value: ApSignature = from_str(
            r#"
              {
                "publicKey": {
                  "id": "https://example.com/",
                  "owner": "https://example.com/",
                  "publicKeyPem": ""
                }
              }
            "#,
        )
        .unwrap();
        let expected = ApSignature {
            public_key: PublicKey {
                id: "https://example.com/".parse().unwrap(),
                owner: "https://example.com/".parse().unwrap(),
                public_key_pem: "".into(),
            },
        };
        assert_eq!(value, expected);
    }

    #[test]
    fn se_custom_person() {
        let actor = ApActor::new("https://example.com/inbox".parse().unwrap(), Person::new());
        let person = CustomPerson::new(
            actor,
            ApSignature {
                public_key: PublicKey {
                    id: "https://example.com/pubkey".parse().unwrap(),
                    owner: "https://example.com/owner".parse().unwrap(),
                    public_key_pem: "pubKeyPem".into(),
                },
            },
        );
        let expected = json!({
            "inbox": "https://example.com/inbox",
            "type": "Person",
            "publicKey": {
                "id": "https://example.com/pubkey",
                "owner": "https://example.com/owner",
                "publicKeyPem": "pubKeyPem"
            }
        });
        assert_eq!(to_value(person).unwrap(), expected);
    }

    #[test]
    fn de_custom_group() {
        let group = CustomGroup::new(
            ApActor::new("https://example.com/inbox".parse().unwrap(), Group::new()),
            ApSignature {
                public_key: PublicKey {
                    id: "https://example.com/pubkey".parse().unwrap(),
                    owner: "https://example.com/owner".parse().unwrap(),
                    public_key_pem: "pubKeyPem".into(),
                },
            },
            SourceProperty {
                source: Source {
                    content: String::from("This is a *custom* group."),
                    media_type: String::from("text/markdown"),
                },
            },
        );
        let expected = json!({
            "inbox": "https://example.com/inbox",
            "type": "Group",
            "publicKey": {
                "id": "https://example.com/pubkey",
                "owner": "https://example.com/owner",
                "publicKeyPem": "pubKeyPem"
            },
            "source": {
                "content": "This is a *custom* group.",
                "mediaType": "text/markdown"
            }
        });
        assert_eq!(to_value(group).unwrap(), expected);
    }

    #[test]
    fn se_licensed_article() {
        let object = ApObject::new(Article::new());
        let licensed_article = LicensedArticle::new(
            object,
            Licensed {
                license: Some("CC-0".into()),
            },
        );
        let expected = json!({
            "type": "Article",
            "license": "CC-0",
        });
        assert_json_eq!(to_value(licensed_article).unwrap(), expected);
    }

    #[test]
    fn de_licensed_article() {
        let value: LicensedArticle = from_str(
            r#"
              {
                "type": "Article",
                "id": "https://plu.me/~/Blog/my-article",
                "attributedTo": ["https://plu.me/@/Admin", "https://plu.me/~/Blog"],
                "content": "Hello.",
                "name": "My Article",
                "summary": "Bye.",
                "source": {
                  "content": "Hello.",
                  "mediaType": "text/markdown"
                },
                "published": "2014-12-12T12:12:12Z",
                "to": ["https://www.w3.org/ns/activitystreams#Public"],
                "license": "CC-0"
             }
           "#,
        )
        .unwrap();
        let expected = json!({
            "type": "Article",
            "id": "https://plu.me/~/Blog/my-article",
            "attributedTo": ["https://plu.me/@/Admin", "https://plu.me/~/Blog"],
            "content": "Hello.",
            "name": "My Article",
            "summary": "Bye.",
            "source": {
                "content": "Hello.",
                "mediaType": "text/markdown"
            },
            "published": "2014-12-12T12:12:12Z",
            "to": ["https://www.w3.org/ns/activitystreams#Public"],
            "license": "CC-0"
        });

        assert_eq!(to_value(value).unwrap(), expected);
    }

    #[test]
    fn de_create_with_licensed_article() {
        let create: Create = from_str(
            r#"
              {
                "id": "https://plu.me/~/Blog/my-article",
                "type": "Create",
                "actor": "https://plu.me/@/Admin",
                "to": "https://www.w3.org/ns/activitystreams#Public",
                "object": {
                   "type": "Article",
                   "id": "https://plu.me/~/Blog/my-article",
                   "attributedTo": ["https://plu.me/@/Admin", "https://plu.me/~/Blog"],
                   "content": "Hello.",
                   "name": "My Article",
                   "summary": "Bye.",
                   "source": {
                     "content": "Hello.",
                     "mediaType": "text/markdown"
                   },
                   "published": "2014-12-12T12:12:12Z",
                   "to": ["https://www.w3.org/ns/activitystreams#Public"],
                   "license": "CC-0"
                 }
               }
            "#,
        )
        .unwrap();
        let base = create.object_field_ref().as_single_base().unwrap();
        let any_base = AnyBase::from_base(base.clone());
        let value = any_base.extend::<LicensedArticle, ArticleType>().unwrap();
        let expected = json!({
            "type": "Article",
            "id": "https://plu.me/~/Blog/my-article",
            "attributedTo": ["https://plu.me/@/Admin", "https://plu.me/~/Blog"],
            "content": "Hello.",
            "name": "My Article",
            "summary": "Bye.",
            "source": {
                "content": "Hello.",
                "mediaType": "text/markdown"
            },
            "published": "2014-12-12T12:12:12Z",
            "to": ["https://www.w3.org/ns/activitystreams#Public"],
            "license": "CC-0"
        });

        assert_eq!(to_value(value).unwrap(), expected);
    }
}
