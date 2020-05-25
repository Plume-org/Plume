use rocket::http::ContentType;
use rocket::response::Content;
use webfinger::*;

use plume_models::{ap_url, blogs::Blog, users::User, PlumeRocket, CONFIG};

#[get("/.well-known/nodeinfo")]
pub fn nodeinfo() -> Content<String> {
    Content(
        ContentType::new("application", "jrd+json"),
        json!({
            "links": [
                {
                    "rel": "http://nodeinfo.diaspora.software/ns/schema/2.0",
                    "href": ap_url(&format!("{domain}/nodeinfo/2.0", domain = CONFIG.base_url.as_str()))
                },
                {
                    "rel": "http://nodeinfo.diaspora.software/ns/schema/2.1",
                    "href": ap_url(&format!("{domain}/nodeinfo/2.1", domain = CONFIG.base_url.as_str()))
                }
            ]
        })
        .to_string(),
    )
}

#[get("/.well-known/host-meta")]
pub fn host_meta() -> String {
    format!(
        r#"
    <?xml version="1.0"?>
    <XRD xmlns="http://docs.oasis-open.org/ns/xri/xrd-1.0">
        <Link rel="lrdd" type="application/xrd+xml" template="{url}"/>
    </XRD>
    "#,
        url = ap_url(&format!(
            "{domain}/.well-known/webfinger?resource={{uri}}",
            domain = CONFIG.base_url.as_str()
        ))
    )
}

struct WebfingerResolver;

#[async_trait::async_trait]
impl AsyncResolver for WebfingerResolver {
    type Repo = PlumeRocket;
    async fn instance_domain<'a>(&self) -> &'a str {
        CONFIG.base_url.as_str()
    }

    async fn find(
        &self,
        prefix: Prefix,
        acct: String,
        ctx: PlumeRocket,
    ) -> Result<Webfinger, ResolverError> {
        match prefix {
            Prefix::Acct => User::find_by_fqn(&ctx, &acct)
                .await
                .and_then(|usr| usr.webfinger(&*ctx.conn))
                .or(Err(ResolverError::NotFound)),
            Prefix::Group => Blog::find_by_fqn(&ctx, &acct)
                .await
                .and_then(|blog| blog.webfinger(&*ctx.conn))
                .or(Err(ResolverError::NotFound)),
            Prefix::Custom(_) => Err(ResolverError::NotFound),
        }
    }
    async fn endpoint<R: Into<String> + Send>(
        &self,
        resource: R,
        resource_repo: PlumeRocket,
    ) -> Result<Webfinger, ResolverError> {
        let resource = resource.into();
        let mut parsed_query = resource.splitn(2, ':');
        let res_prefix = Prefix::from(parsed_query.next().ok_or(ResolverError::InvalidResource)?);
        let res = parsed_query.next().ok_or(ResolverError::InvalidResource)?;

        let mut parsed_res = res.splitn(2, '@');
        let user = parsed_res.next().ok_or(ResolverError::InvalidResource)?;
        let domain = parsed_res.next().ok_or(ResolverError::InvalidResource)?;
        if domain == self.instance_domain().await {
            self.find(res_prefix, user.to_string(), resource_repo).await
        } else {
            Err(ResolverError::WrongDomain)
        }
    }
}

#[get("/.well-known/webfinger?<resource>")]
pub async fn webfinger(resource: String, rockets: PlumeRocket) -> Content<String> {
    let wf_resolver = WebfingerResolver;
    match wf_resolver
        .endpoint(resource, rockets)
        .await
        .and_then(|wf| serde_json::to_string(&wf).map_err(|_| ResolverError::NotFound))
    {
        Ok(wf) => Content(ContentType::new("application", "jrd+json"), wf),
        Err(err) => Content(
            ContentType::new("text", "plain"),
            String::from(match err {
                ResolverError::InvalidResource => {
                    "Invalid resource. Make sure to request an acct: URI"
                }
                ResolverError::NotFound => "Requested resource was not found",
                ResolverError::WrongDomain => "This is not the instance of the requested resource",
            }),
        ),
    }
}
