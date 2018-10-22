# API documentation

## Getting an API token

To get access to the API, you should register your app and obtain a
token. To do so, use the `/api/v1/apps` API (accessible without a token) to create
a new app. Store the result somewhere for future use.

Then send a request to `/api/v1/oauth2`, with the following GET parameters:

- `client_id`, your client ID.
- `client_secret`, your client secret.
- `scopes`, the scopes you want to access. They are separated by `+`, and can either
be `read` (global read), `write` (global write), `read:SCOPE` (read only in `SCOPE`),
or `write:SCOPE` (write only in `SCOPE`).
- `username` the username (not the email, display name nor the fully qualified name) of the
user using your app.
- `password`, the password of the user.

Plume will respond with something similar to:

```json
{
  "token": "<YOUR TOKEN HERE>"
}
```

To authenticate your requests you should put this token in the `Authorization` header:

```
Authorization: Bearer <YOUR TOKEN HERE>
```

<script src="//unpkg.com/swagger-ui-dist@3/swagger-ui-bundle.js"></script>

<div id="api"></div>

<script>
const ui = SwaggerUIBundle({
    url: "/Plume/api.yaml",
    dom_id: '#api',
    presets: [
        SwaggerUIBundle.presets.apis,
        SwaggerUIBundle.SwaggerUIStandalonePreset
    ],
    layout: "StandaloneLayout"
})
</script>
