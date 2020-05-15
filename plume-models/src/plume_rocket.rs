pub use self::module::PlumeRocket;

#[cfg(not(test))]
mod module {
    use crate::{db_conn::DbConn, search, users};
    use rocket::{
        request::{self, FlashMessage, FromRequest, Request},
        Outcome, State,
    };
    use scheduled_thread_pool::ScheduledThreadPool;
    use std::sync::Arc;

    /// Common context needed by most routes and operations on models
    pub struct PlumeRocket {
        pub conn: DbConn,
        pub intl: rocket_i18n::I18n,
        pub user: Option<users::User>,
        pub searcher: Arc<search::Searcher>,
        pub worker: Arc<ScheduledThreadPool>,
        pub flash_msg: Option<(String, String)>,
    }

    #[rocket::async_trait]
    impl<'a, 'r> FromRequest<'a, 'r> for PlumeRocket {
        type Error = ();

        async fn from_request(request: &'a Request<'r>) -> request::Outcome<Self, Self::Error> {
            let conn = DbConn::from_request(request).await.succeeded().unwrap();
            let intl = rocket_i18n::I18n::from_request(request)
                .await
                .succeeded()
                .unwrap();
            let user = users::User::from_request(request)
                .await
                .succeeded()
                .unwrap();
            let worker = request
                .guard::<State<'_, Arc<ScheduledThreadPool>>>()
                .await
                .succeeded()
                .unwrap();
            let searcher = request
                .guard::<State<'_, Arc<search::Searcher>>>()
                .await
                .succeeded()
                .unwrap();
            let flash_msg = request.guard::<FlashMessage<'_, '_>>().await.succeeded();
            Outcome::Success(PlumeRocket {
                conn,
                intl,
                user: Some(user),
                flash_msg: flash_msg.map(|f| (f.name().into(), f.msg().into())),
                worker: worker.clone(),
                searcher: searcher.clone(),
            })
        }
    }
}

#[cfg(test)]
mod module {
    use crate::{db_conn::DbConn, search, users};
    use rocket::{
        request::{self, FromRequest, Request},
        Outcome, State,
    };
    use scheduled_thread_pool::ScheduledThreadPool;
    use std::sync::Arc;

    /// Common context needed by most routes and operations on models
    pub struct PlumeRocket {
        pub conn: DbConn,
        pub user: Option<users::User>,
        pub searcher: Arc<search::Searcher>,
        pub worker: Arc<ScheduledThreadPool>,
    }

    #[rocket::async_trait]
    impl<'a, 'r> FromRequest<'a, 'r> for PlumeRocket {
        type Error = ();

        async fn from_request(request: &'a Request<'r>) -> request::Outcome<Self, Self::Error> {
            let conn = try_outcome!(DbConn::from_request(request).await);
            let user = try_outcome!(users::User::from_request(request).await);
            let worker = try_outcome!(request.guard::<'_, State<'_, Arc<ScheduledThreadPool>>>());
            let searcher = try_outcome!(request.guard::<'_, State<'_, Arc<search::Searcher>>>());
            Outcome::Success(PlumeRocket {
                conn,
                user: Some(user),
                worker: worker.clone(),
                searcher: searcher.clone(),
            })
        }
    }
}
