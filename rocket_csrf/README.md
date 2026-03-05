# rocket_csrf

From [rocket_csrf_token](https://github.com/wiseaidev/rocket_csrf_token)

## Usage

Attach [fairing](https://rocket.rs/guide/v0.5/fairings/#fairings) to the Rocket instance:

```rust
#![feature(decl_macro)]

#[macro_use]
extern crate rocket;
#[macro_use]
extern crate serde_derive;

use rocket_dyn_templates::Template;

#[launch]
fn rocket() -> _ {
    rocket::build()
    .attach(rocket_csrf_token::Fairing::default())
    .attach(Template::fairing())
    .mount("/", routes![new, create])
}
```

You also can configure [fairing](https://rocket.rs/guide/v0.5/fairings/#fairings):

```rust
#[launch]
fn rocket() -> _ {
  rocket::build()
    .attach(
      rocket_csrf_token::Fairing::new(
        rocket_csrf_token::CsrfConfig
          ::default()
          .with_cookie_name("foobar")
          .with_cookie_len(64)
          .with_lifetime(Some(rocket::time::Duration::days(3)))
      )
    )
    .attach(Template::fairing())
    .mount("/", routes![new, create])
}
```

Add [guard](https://rocket.rs/guide/v0.5/requests/#request-guards) to any request where you want to have access to session's CSRF token (e.g. to include it in forms) or verify it (e.g. to validate form):

```rust
use rocket::form::Form;
use rocket::response::Redirect;
use rocket_csrf_token::CsrfToken;
use rocket_dyn_templates::Template;

#[get("/comments/new")]
fn new(csrf_token: CsrfToken) -> Template {
  // your code
}

#[post("/comments", data = "<form>")]
fn create(csrf_token: CsrfToken, form: Form<Comment>) -> Redirect {
  // your code
}
```

Get CSRF token from [guard](https://rocket.rs/guide/v0.5/requests/#request-guards) to use it in [templates](https://rocket.rs/guide/v0.5/responses/#templates):

```rust
#[get("/comments/new")]
fn new(csrf_token: CsrfToken) -> Template {
  let authenticity_token: &str = csrf_token.authenticity_token();

  // your code
}
```

Add CSRF token to your HTML forms in [templates](https://rocket.rs/guide/v0.5/responses/#templates):

```html
<form method="post" action="/comments">
  <input
    type="hidden"
    name="authenticity_token"
    value="{{ authenticity_token }}"
  />
  <!-- your fields -->
</form>
```

Add attribute `authenticity_token` to your [forms](https://rocket.rs/guide/v0.5/requests/#forms):

```rust
#[derive(FromForm)]
struct Comment {
  authenticity_token: String,
  // your attributes
}
```

Validate [forms](https://rocket.rs/guide/v0.5/requests/#forms) to have valid authenticity token:

```rust
#[post("/comments", data = "<form>")]
fn create(csrf_token: CsrfToken, form: Form<Comment>) -> Redirect {
  if let Err(_) = csrf_token.verify(&form.authenticity_token) {
    return Redirect::to(uri!(new));
  }

  // your code
}
```

See the complete code in [minimal example](examples/minimal).

## TODO

- [X] Add fairing to verify all requests as an option.
- [X] Verify `X-CSRF-Token` header.
- [X] Set cookie to expire with session.
- [ ] Add [data guard](https://api.rocket.rs/v0.5-rc/rocket/data/trait.FromData.html) to verify forms with a guard.
- [ ] Add helpers to render form field.
- [ ] Add helpers to add HTML meta tags for Ajax with `X-CSRF-Token` header (WIP).
- [ ] Use authenticity token encryption from [Ruby on Rails](https://github.com/rails/rails/blob/v6.0.3.4/actionpack/lib/action_controller/metal/request_forgery_protection.rb).
- [ ] Allow to configure CSRF protection (CSRF token byte length, cookie name, etc.).
