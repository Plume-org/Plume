# How Plume Federates

To federate with other Fediverse software (and itself), Plume uses various 
protocols:
- [ActivityPub](http://activitypub.rocks/), as the main federation protocol.
- [WebFinger](https://webfinger.net/), to find other users and blog easily.
- [HTTP Signatures](https://tools.ietf.org/id/draft-cavage-http-signatures-01.html), to 
authenticate activities.
- [NodeInfo](http://nodeinfo.diaspora.software/), which is not part of the 
federation itself, but that gives some metadata about each instance.

Currently, the following are federated:
- User profiles
- Blogs
- Articles
- Comments
- Likes
- Reshares

And these parts are not federated, but may be in the future:
- Media gallery
- Instance metadata

## WebFinger

WebFinger is used to discover remote profiles. When you open the page of an unknown 
user (`/@/username@instance.tld`),
Plume will send a WebFinger request to the other instance, on the standard 
`/.well-known/webfinger` endpoint. Plume
will ignore the `/.well-known/host-meta` endpoint (that can normally be used to 
define another WebFinger endpoint),
and always use the standard URL.

Plume uses the [`webfinger`](https://crates.io/crates/webfinger) crate to serve 
WebFinger informations and fetch them.

## HTTP Signatures

Plume check that each incoming Activity has been signed with the `actor`'s keypair.

To achieve that, it uses the `Signature` HTTP header. For more details on how this 
header is generated, please refer to the [HTTP Signatures 
Specification](https://tools.ietf.org/id/draft-cavage-http-signatures-01.html).

The `Digest` header should be present too, and used to generate the signature, so 
that we can verify the body of the request too.

## NodeInfo

Plume exposes instance metadata with NodeInfo on the `/nodeinfo` URL.

*Example output*

```json
{
  "version": "2.0",
  "software": {
    "name": "Plume",
    "version": "0.2.0"
  },
  "protocols": ["activitypub"],
  "services": {
    "inbound": [],
    "outbound": []
  },
  "openRegistrations": true,
  "usage": {
    "users": {
      "total": 42
    },
    "localPosts": 7878,
    "localComments": 1312
  },
  "metadata": {}
}
```

## ActivityPub

Each user has a personal inbox at `/@/username/inbox`, and each instance has a shared
inbox at `/inbox`.

If available, Plume will use the shared inbox to deliver activities.

### Object representation

- `Note` represents a comment.
- `Article` is an article.
- `Person` is for users.
- `Group` is for blogs.

### Supported Activities

Plume 0.2.0 supports the following activity types.

#### Accept

Accepts a follow request.

It will be ignored when received, as Plume considered follow requests to be 
immediatly approved in all cases (however, this will change in the future).

When a [`Follow`](#follow) activity is received, Plume will respond with this 
activity.

- `actor` is the ID of the user accepting the request.
- `object` is the `Follow` object being accepted.

#### Announce

Reshares an article (not available for other objects).

Makes an user (`actor`) reshare a post (`object`).
- `actor` is the ID of the user who reshared the post.
- `object` is the ID of the post to reshare.

#### Create

Creates a new article or comment.

If `object` is an `Article`:
- `object.attibutedTo` is a list containing the ID of the authors and of the blog 
in which this article have been published. If no blog ID is specified, the article 
will be rejected. The `actor` of the activity corresponds to the user that clicked 
the "Publish" button, and should normally be one of the author in `attributedTo`.
- `object.name` is the title of the article.
- `object.content` is a string containing the HTML of the rendered article.
- `object.creationDate` is the date of the first publication of this article.
- `object.source` is a `Source` object, and its content is the Markdown source of 
this article.
- `object.tag` is a list, and its elements are either:
    - a `Hashtag` object, for the tag of the article (no difference is made between 
global tags shown at the end of the article and hashtags in the article itself for 
the
moment).
    - a `Mention` object, for every actor that have been mentionned in this 
article.

If `object` is a `Note`:
- `object.content` is the HTML source of the rendered comment.
- `object.inReplyTo` is the ID of the previous comment in the thread, or of the 
post that is commented if there is no previous comment.
- `object.spoilerText` is a string to be displayed in place of the comment, unless 
the reader explicitely express their will to see the actual content (what is called 
*Content Warning* in Mastodon)
- `object.tag` is a list of `Mention` that correspond to the mentionned users.

#### Delete

Deletes an object that was first created with a `Create` activity.

`object` is a `Tombstone`, and `object.id` the ID of the object to delete (either 
an Article ID, or a Note ID).

#### Follow

When received, the actor is added to the follower list of the target.

These activities are immediatly accepted (see [`Accept`](#accept)) by Plume.

For blogs, they won't actually do anything else than sending back an `Accept` 
activity: following a blog is not yet implemented.

- `actor` is the ID of an Actor, or a `Person` object. It represent the new 
follower.
- `object` is the ID of the target user or blog.

#### Like

Can be used to add a like to an article.

- `actor` is the ID of the user liking the article.
- `object` is the ID of the post being liked.

#### Update

Updates an article.

- `object` is an `Article` object. It has no mandatory field other than `id`. Only 
present fields will be updated.
- `object.id` is the ID the of the article being updated.
- `object.title` is the new title of the article.
- `object.content` is the updated HTML of the article.
- `object.subtitle` is the updated subtitle of the article.
- `object.source` is a `Source` object, and its `content` property is the updated 
markdown of the article.

#### Undo

Cancels a previous action (either a like, reshare or follow).

- `object` is the `Announce`, `Follow` or `Like` to undo.
