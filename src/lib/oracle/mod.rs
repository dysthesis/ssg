use crate::{ctx::Ctx, query::Id};

/// An oracle is just a function which is used as a heruistic to determine if
/// a query should be re-built, _i.e._ if the cache is stale.
pub trait Oracle {
    fn should_rebuild(query: &Id, ctx: &Ctx) -> bool;
}
