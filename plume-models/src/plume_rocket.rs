pub use self::module::PlumeRocket;

#[cfg(not(test))]
mod module {
    use crate::{search, users};
    use rocket::{
        request::{FlashMessage, FromRequest, Outcome, Request},
        State,
    };
    use scheduled_thread_pool::ScheduledThreadPool;
    use std::sync::Arc;

    /// Common context needed by most routes and operations on models
    pub struct PlumeRocket {
        pub intl: rocket_i18n::I18n,
        pub user: Option<users::User>,
        pub searcher: Arc<search::Searcher>,
        pub worker: Arc<ScheduledThreadPool>,
        pub flash_msg: Option<(String, String)>,
    }

    #[rocket::async_trait]
    impl<'r> FromRequest<'r> for PlumeRocket {
        type Error = ();

        async fn from_request(request: &'r Request<'_>) -> Outcome<PlumeRocket, Self::Error> {
            let guard_intl = request.guard::<rocket_i18n::I18n>().await;
            let guard_worker = request.guard::<&State<Arc<ScheduledThreadPool>>>().await;
            let guard_searcher = request.guard::<&State<Arc<search::Searcher>>>().await;

            let user = request.guard::<users::User>().await.succeeded();
            let flash_msg = request.guard::<FlashMessage<'_>>().await.succeeded();

            guard_intl.and_then(|intl| {
                guard_worker.and_then(|worker| {
                    guard_searcher.map(|searcher| {
                        PlumeRocket {
                            intl,
                            user,
                            flash_msg: flash_msg.map(|f| (f.kind().into(), f.message().into())),
                            worker: (*worker).clone(),
                            searcher: (*searcher).clone(),
                        }
                    })
                })
            })
        }
    }
}

#[cfg(test)]
mod module {
    use crate::{search, users};
    use rocket::{
        request::{self, FromRequest, Request},
        Outcome, State,
    };
    use scheduled_thread_pool::ScheduledThreadPool;
    use std::sync::Arc;

    /// Common context needed by most routes and operations on models
    pub struct PlumeRocket {
        pub user: Option<users::User>,
        pub searcher: Arc<search::Searcher>,
        pub worker: Arc<ScheduledThreadPool>,
    }

    #[rocket::async_trait]
    impl<'r> FromRequest<'r> for PlumeRocket {
        type Error = ();

        async fn from_request(request: &'r Request<'_>) -> Outcome<PlumeRocket, Self::Error> {
            let user = request.guard::<users::User>().succeeded();
            let worker = request.guard::<'_, State<'_, Arc<ScheduledThreadPool>>>()?;
            let searcher = request.guard::<'_, State<'_, Arc<search::Searcher>>>()?;
            Outcome::Success(PlumeRocket {
                user,
                worker: worker.clone(),
                searcher: searcher.clone(),
            })
        }
    }
}
