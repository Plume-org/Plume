pub use self::module::PlumeRocket;

#[cfg(not(test))]
mod module {
    use crate::db_conn::DbConn;
    use crate::search;
    use crate::users;
    use rocket::{
        request::{self, FromRequest, Request},
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
    }

    impl<'a, 'r> FromRequest<'a, 'r> for PlumeRocket {
        type Error = ();

        fn from_request(request: &'a Request<'r>) -> request::Outcome<PlumeRocket, ()> {
            let conn = request.guard::<DbConn>()?;
            let intl = request.guard::<rocket_i18n::I18n>()?;
            let user = request.guard::<users::User>().succeeded();
            let worker = request.guard::<State<Arc<ScheduledThreadPool>>>()?;
            let searcher = request.guard::<State<Arc<search::Searcher>>>()?;
            Outcome::Success(PlumeRocket {
                conn,
                intl,
                user,
                worker: worker.clone(),
                searcher: searcher.clone(),
            })
        }
    }
}

#[cfg(test)]
mod module {
    use crate::db_conn::DbConn;
    use crate::search;
    use crate::users;
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

    impl<'a, 'r> FromRequest<'a, 'r> for PlumeRocket {
        type Error = ();

        fn from_request(request: &'a Request<'r>) -> request::Outcome<PlumeRocket, ()> {
            let conn = request.guard::<DbConn>()?;
            let user = request.guard::<users::User>().succeeded();
            let worker = request.guard::<State<Arc<ScheduledThreadPool>>>()?;
            let searcher = request.guard::<State<Arc<search::Searcher>>>()?;
            Outcome::Success(PlumeRocket {
                conn,
                user,
                worker: worker.clone(),
                searcher: searcher.clone(),
            })
        }
    }
}
