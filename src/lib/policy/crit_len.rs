use crate::{ctx::Ctx, policy::Policy, query};

pub struct CritLen {}
impl Policy for CritLen {
    fn score(query: query::Id, ctx: &Ctx) -> usize {
        todo!()
    }
}
