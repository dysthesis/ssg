use crate::{ctx::Global, policy::Policy, query};

pub struct CritLen {}
impl Policy for CritLen {
    fn score(query: query::Id, ctx: &Global) -> usize {
        todo!()
    }
}
