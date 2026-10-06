use std::{collections::BinaryHeap, marker::PhantomData, pin::Pin};

use crate::{ctx::Ctx, graph::Graph, policy::Policy, query, store};

/// A unit of work for [`Scheduler`]
pub struct Task {
    /// A reference to the [`query::Query`] which produced this work.
    pub query: query::Id,
    /// The [`Future`] that corresponds with this [`query::Query`]
    pub fut: Pin<Box<dyn Future<Output = store::Id> + Send>>,
}

impl Task {
    pub fn new(query: query::Id, ctx: &Ctx) -> Self {
        // let query = ctx.
        todo!()
    }
}

/// Bookkeeping to keep track of which tasks can be run next.
#[derive(Default)]
pub struct Scheduler<P: Policy> {
    ready: BinaryHeap<Task>,
    blocked: Vec<Task>,
    graph: Graph,
    _policy: PhantomData<P>,
}

impl<P: Policy> Scheduler<P> {
    pub fn new() -> Self {
        let ready = BinaryHeap::new();
        let blocked = Vec::new();
        let graph = Graph::new();
        Self {
            ready,
            blocked,
            graph,
            _policy: PhantomData::<P>,
        }
    }
    /// Submit a task to schedule. This is usually interfaced to by [`Ctx`]
    /// whenever someone requests a query, or by [`Runtime`]
    pub(crate) fn submit(&self, task: Task) {
        todo!()
    }

    /// Block until a runnable task exists.
    pub(crate) fn next(&self) -> Option<Task> {
        todo!()
    }

    /// The worker polled this task and got Pending.
    pub(crate) fn park(&self, task: Task) {
        todo!()
    }

    /// Something awaited by this task became ready, so move it to [`Self::ready`]
    pub(crate) fn wake(&self, id: &query::Id) {
        todo!()
    }

    /// The task has been completed
    pub(crate) fn complete(&self, task: Task, output: store::Id) {
        // Wake all of the dependent tasks
        for task in self.graph.dependents_of(task.query) {
            self.wake(task);
        }
        todo!()
    }
}
