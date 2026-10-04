use crate::{ctx::Ctx, query};

pub mod crit_len;
/// How [`Task`] are scored, which determines how they are prioritised by
/// [`Scheduler`].
pub trait Policy {
    fn score(query: query::Id, ctx: &Ctx) -> usize;
}
