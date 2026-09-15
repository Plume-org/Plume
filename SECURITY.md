# Security Policy

## Supported versions

Plume is developed on the `main` branch. Security fixes are applied to `main`
and released in the latest stable minor version (currently the `0.7.x`
series). Older releases do not receive security patches.

| Version          | Supported          |
| ---------------- | ------------------ |
| `main` (0.7.x)   | :white_check_mark: |
| < 0.7            | :x:                |

## Reporting a vulnerability

**Please do not open a public issue for security problems.**

Use one of the following private channels:

- Open a [private security advisory][gh-advisory] on the project repository
  (Security tab → "Report a vulnerability").
- If you cannot use GitHub, contact the maintainers directly through the
  project's Matrix room (`#plume-blog:matrix.org`) and ask for a private
  channel. Do not include vulnerability details in the public room.

Please include, when possible:

- the affected version/commit,
- a description of the issue and its impact,
- reproduction steps or a proof of concept,
- any suggested fix.

We will acknowledge receipt as soon as possible, keep you updated while the
issue is investigated, and credit you in the advisory unless you prefer to
stay anonymous. Please give us a reasonable amount of time to release a fix
before any public disclosure.

[gh-advisory]: https://github.com/Plume-org/Plume/security/advisories/new

## Security model

Plume is a federated server: anyone on the network can send it signed
ActivityPub activities, and content coming from remote instances is
untrusted. The following properties are expected:

- **Authentication**: users are authenticated with a signed session cookie.
  Passwords are hashed with bcrypt. API tokens are random 256-bit values.
- **Authorization**: a user may only act on their own account, their own
  media, their blogs (as an author) and their own posts/comments. Remote
  activities are only accepted when signed by the actor they claim to come
  from, and the actor must be allowed to perform the action.
- **CSRF**: all state-changing browser routes are protected by the
  `rocket_csrf` fairing. Only the ActivityPub inboxes and the token-
  authenticated API are exempted.
- **Output encoding**: user supplied HTML (articles, summaries, comments) is
  sanitized with `ammonia` before being stored/rendered. Templates
  auto-escape by default.
- **Federation requests**: outbound HTTP requests must never reach loopback,
  private, link-local or other special-purpose addresses.

## Security fixes

The following vulnerabilities were identified in a security review of
`0.7.3-dev` and fixed. The corresponding tests were added or updated.

### 1. Remote post overwrite through `Create` activities (CWE-862)

`Post::from_activity` (in `plume-models/src/posts.rs`) updated any post whose
`ap_url` matched the `url` field of an incoming `Create` object. Because
`id` and `url` are chosen by the sender, a remote actor could overwrite the
title, content, license, cover and source of **any known post**, including
local ones.

*Fix*: an incoming `Create` activity never modifies an existing post. Posts
can only be changed by their authors, through a signed `Update` activity
(which was already checked in `PostUpdate::activity`).

### 2. Cross-instance post injection and author spoofing (CWE-290, CWE-639)

`Post::from_activity` built the author list and the target blog from the
`attributedTo` field of the remote object. A remote actor could therefore
create a post inside a blog of another instance (including local blogs) and
list arbitrary users (including local ones) as its authors.

*Fix*: `FromId::from_id_with_actor` was added to the inbox machinery and is
used before an object is created. For posts, when the target blog is already
known locally, the actor must belong to the same instance as that blog; a
post can not be created in a local blog by a remote actor. Local users are
additionally removed from the author list of posts belonging to remote blogs.

### 3. Local account/blog impersonation from remote activities
(CWE-290)

`User::from_activity` and `Blog::from_activity` created database records even
when the `id` host was the local instance, allowing a remote activity to
create a "local" user (with an attacker-controlled public key, and therefore
to impersonate a local account) or a "local" blog.

*Fix*: creating local users or blogs from a remote activity is now rejected
with `Error::Unauthorized`.

### 4. Missing authorization on `POST /api/v1/posts` (CWE-862)

The API endpoint only checked the token scope (`write:posts`) but did not
check that the token owner was an author of the target blog, allowing any
user with an API token to publish in any blog.

*Fix*: the endpoint now requires the token owner to be an author of the blog
designated by `blog_id` (or of their only blog when `blog_id` is omitted).

### 5. Stored XSS through uploaded and mirrored media (CWE-434, CWE-79)

Uploads accepted almost any file extension whose name was alphanumeric, and
mirrored remote media kept the extension of the remote URL. Files such as
`.html` or `.svg` were thus served from the instance domain with an active
`Content-Type`, allowing stored cross-site scripting (including session
theft through a malicious upload).

*Fix*: only a whitelist of media extensions that cannot be interpreted as
active content is accepted (`plume-models/src/medias.rs`,
`ALLOWED_MEDIA_EXTENSIONS`). The same whitelist is applied to mirrored
remote media, unknown extensions fall back to `png`, mirrored files use a
sanitized path, and the `Content-Type` used for S3 storage is derived from
the extension instead of the client/remote metadata. A
`X-Content-Type-Options: nosniff` header is now sent on every response (see
below).

### 6. Denial of service through a malformed `Signature` header (CWE-20)

`verify_http_headers` (in `plume-common/src/activity_pub/sign.rs`) sliced the
`Signature` header without bounds checks, so values such as `keyId=` caused a
panic on every inbox request carrying them.

*Fix*: header parameters are now extracted with bounds-checked helpers;
malformed headers simply fail signature verification.

### 7. Server-side request forgery in federation requests (CWE-918)

Outbound requests (`request::get` and `broadcast`) followed arbitrary URLs,
including redirects, without filtering loopback/private addresses. A remote
actor could therefore make the instance fetch internal services (e.g. cloud
metadata endpoints) and, for mirrored media, read the response through the
public media URL.

*Fix*: `plume-common/src/activity_pub/request.rs` now validates that URLs use
`http(s)` and resolves their host, rejecting loopback, private, link-local,
multicast, CGNAT and other special-purpose ranges. The validated address is
pinned for the request (`ClientBuilder::resolve`) to prevent DNS rebinding,
and a custom redirect policy validates every redirect target, with a limit of
5 redirects. Inboxes that do not pass validation are ignored when
broadcasting.

### 8. Panics on unauthenticated requests (CWE-248)

Several routes used `rockets.user.clone().unwrap()` without a `User` guard
(`posts::new`, `posts::edit`, `posts::update`, `posts::create`,
`posts::delete`, `blogs::create`), so unauthenticated requests panicked
instead of being redirected.

*Fix*: these routes now take a `User` request guard. `posts::edit_auth` was
added so that unauthenticated users are redirected to the login page, like
the other routes.

### 9. Information disclosure of non-public comments (CWE-200)

`GET /~/<blog>/<slug>/comment/<id>` returned the ActivityPub representation
of any comment, including comments that are not publicly visible.

*Fix*: only publicly visible comments are returned.

### 10. Panics while processing remote outbox responses (CWE-20)

`User::fetch_outbox` and `User::fetch_outbox_page` unwrapped `next`/`first`
fields without checking their type, letting a remote instance panic the
server by returning a non-string value.

*Fix*: these fields are only used when they are strings.

### 11. LDAP distinguished name injection (CWE-90)

LDAP user names were interpolated into a bind DN without escaping, letting a
user-controlled name change the structure of the DN.

*Fix*: values are escaped according to RFC 4514 before being inserted in a
DN.

### 12. Open redirect through WebFinger templates (CWE-601)

The remote interaction endpoints redirected to a URI taken from a remote
WebFinger response without validating its scheme.

*Fix*: redirect targets are validated (http/https, no local/private host)
with `is_safe_redirect_uri` before use.

### 13. Hardening

- Session cookies are now `HttpOnly`, `SameSite=Lax` and, when Rocket runs in
  the `production` environment, `Secure`.
- Every response includes `X-Content-Type-Options: nosniff`,
  `X-Frame-Options: SAMEORIGIN` and `Referrer-Policy: same-origin`.
- Client-secret comparison in the OAuth endpoint is constant-time.
- Usernames created through the email signup flow are validated like the
  password signup flow.
- Theme names set by users or blogs must exist on the instance
  (`Instance::list_themes`).

## Hardening recommendations for operators

- Serve Plume behind HTTPS and set `ROCKET_ENV=production` (or use Rocket's
  production profile) so that the `Secure` flag is set on session cookies.
- Set a strong `ROCKET_SECRET_KEY` (`openssl rand -base64 32`); all private
  cookies and CSRF tokens depend on it.
- Keep `MEDIA_UPLOAD_DIRECTORY`/`static/media` writable only by Plume.
- Run Plume as an unprivileged user and keep the database and OpenSSL
  packages up to date.
- Do not enable `S3_DIRECT_DOWNLOAD`/`S3_ALIAS_HOST` unless the bucket is
  configured with a safe `Content-Type` policy; Plume now derives the content
  type itself but old objects may still be misconfigured.
- The federation client refuses to fetch private/loopback addresses. If you
  need to federate with instances on a private network, do it at your own
  risk: this protection cannot be disabled.

## Known limitations

- **WebFinger resolution** still uses the `webfinger` crate for some flows
  and therefore does not go through the SSRF-guarded HTTP client. The impact
  is limited (the requested URL is fixed to
  `/.well-known/webfinger?resource=...` and the response is not echoed back),
  but this is a known gap.
- **Replay protection**: HTTP signatures are checked against a 12-hour
  window but there is no server-side nonce cache, so captured requests can be
  replayed within that window. Federation peers are expected to sign the
  `(request-target)` and `date` headers, which limits the impact.
- **Federation of remote blogs**: since Plume does not import the author list
  of remote blogs, it cannot verify that a remote actor is an author of a
  remote blog it already knows; it only checks that the actor and the blog
  come from the same instance.
- **Rate limiting** is not implemented (registration, password reset, login
  attempts, API). Deployments exposed to the Internet should add rate
  limiting at the reverse-proxy level.
- Media uploaded before this audit was not restricted: check your existing
  `static/media` directory for unexpected extensions.
