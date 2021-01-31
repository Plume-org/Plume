pub use self::module::PlumeRocket;

#[cfg(not(test))]
mod module {
    use crate::{search, users};
    use rocket::{
        request::{self, FlashMessage, FromRequest, Request},
        Outcome, State,
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

    impl<'a, 'r> FromRequest<'a, 'r> for PlumeRocket {
        type Error = ();

        fn from_request(request: &'a Request<'r>) -> request::Outcome<PlumeRocket, ()> {
            let intl = request.guard::<rocket_i18n::I18n>()?;
            let user = request.guard::<users::User>().succeeded();
            let worker = request.guard::<'_, State<'_, Arc<ScheduledThreadPool>>>()?;
            let searcher = request.guard::<'_, State<'_, Arc<search::Searcher>>>()?;
            let flash_msg = request.guard::<FlashMessage<'_, '_>>().succeeded();
            Outcome::Success(PlumeRocket {
                intl,
                user,
                flash_msg: flash_msg.map(|f| (f.name().into(), f.msg().into())),
                worker: worker.clone(),
                searcher: searcher.clone(),
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

    impl<'a, 'r> FromRequest<'a, 'r> for PlumeRocket {
        type Error = ();

        fn from_request(request: &'a Request<'r>) -> request::Outcome<PlumeRocket, ()> {
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
