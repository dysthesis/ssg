use smallvec::SmallVec;

use crate::query;

pub const CHUNK_SIZE: usize = 4; // Parameter to tune
pub type Chunk = [query::Id; CHUNK_SIZE];

#[derive(Default)]
pub struct Graph {
    inner: Vec<SmallVec<Chunk>>,
}

impl Graph {
    pub fn new() -> Self {
        Self { inner: Vec::new() }
    }
    pub fn add_dependency(&mut self, dependency: query::Id, dependent: query::Id) {
        self.inner[usize::from(dependency)].push(dependent);
    }

    pub fn dependents_of(&self, dependency: query::Id) -> &[query::Id] {
        &self.inner[usize::from(dependency)]
    }
}
