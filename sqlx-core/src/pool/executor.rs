use either::Either;
use futures_core::future::BoxFuture;
use futures_core::stream::BoxStream;
use futures_util::TryStreamExt;

use crate::database::{Database, HasStatement};
use crate::describe::Describe;
use crate::error::Error;
use crate::executor::{Execute, Executor};
use crate::pool::Pool;
use logging_timer::{time};

impl<'p, DB: Database> Executor<'p> for &'_ Pool<DB>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
{
    type Database = DB;

    fn fetch_many<'e, 'q: 'e, E: 'q>(
        self,
        query: E,
    ) -> BoxStream<'e, Result<Either<DB::QueryResult, DB::Row>, Error>>
    where
        E: Execute<'q, Self::Database>,
    {
        let pool = self.clone();

        Box::pin(try_stream! {
            let mut conn = pool.acquire().await?;
            let mut s = conn.fetch_many(query);

            while let Some(v) = s.try_next().await? {
                r#yield!(v);
            }

            Ok(())
        })
    }

    #[time]
    fn fetch_optional<'e, 'q: 'e, E: 'q>(
        self,
        query: E,
    ) -> BoxFuture<'e, Result<Option<DB::Row>, Error>>
    where
        E: Execute<'q, Self::Database>,
    {
        let pool = self.clone();

        Box::pin(async move { pool.acquire().await?.fetch_optional(query).await })
    }

    fn prepare_with<'e, 'q: 'e>(
        self,
        sql: &'q str,
        parameters: &'e [<Self::Database as Database>::TypeInfo],
    ) -> BoxFuture<'e, Result<<Self::Database as HasStatement<'q>>::Statement, Error>> {
        let pool = self.clone();

        Box::pin(async move { pool.acquire().await?.prepare_with(sql, parameters).await })
    }

    #[doc(hidden)]
    fn describe<'e, 'q: 'e>(
        self,
        sql: &'q str,
    ) -> BoxFuture<'e, Result<Describe<Self::Database>, Error>> {
        let pool = self.clone();

        Box::pin(async move { pool.acquire().await?.describe(sql).await })
    }
}

// NOTE: required due to lack of lazy normalization
#[allow(unused_macros)]
macro_rules! impl_executor_for_pool_connection {
    ($DB:ident, $C:ident, $R:ident) => {
        impl<'c> crate::executor::Executor<'c> for &'c mut crate::pool::PoolConnection<$DB> {
            type Database = $DB;

            #[inline]
            fn fetch_many<'e, 'q: 'e, E: 'q>(
                self,
                query: E,
            ) -> futures_core::stream::BoxStream<
                'e,
                Result<
                    either::Either<<$DB as crate::database::Database>::QueryResult, $R>,
                    crate::error::Error,
                >,
            >
            where
                'c: 'e,
                E: crate::executor::Execute<'q, $DB>,
            {
                (**self).fetch_many(query)
            }

            #[inline]
            fn fetch_optional<'e, 'q: 'e, E: 'q>(
                self,
                query: E,
            ) -> futures_core::future::BoxFuture<'e, Result<Option<$R>, crate::error::Error>>
            where
                'c: 'e,
                E: crate::executor::Execute<'q, $DB>,
            {
                (**self).fetch_optional(query)
            }

            #[inline]
            fn prepare_with<'e, 'q: 'e>(
                self,
                sql: &'q str,
                parameters: &'e [<$DB as crate::database::Database>::TypeInfo],
            ) -> futures_core::future::BoxFuture<
                'e,
                Result<<$DB as crate::database::HasStatement<'q>>::Statement, crate::error::Error>,
            >
            where
                'c: 'e,
            {
                (**self).prepare_with(sql, parameters)
            }

            #[doc(hidden)]
            #[inline]
            fn describe<'e, 'q: 'e>(
                self,
                sql: &'q str,
            ) -> futures_core::future::BoxFuture<
                'e,
                Result<crate::describe::Describe<$DB>, crate::error::Error>,
            >
            where
                'c: 'e,
            {
                (**self).describe(sql)
            }
        }
    };
}
