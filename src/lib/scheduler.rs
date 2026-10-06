use std::{
    collections::{BinaryHeap, HashMap},
    marker::PhantomData,
    pin::Pin,
};

use crate::{
    ctx::Global,
    graph::Graph,
    policy::Policy,
    query,
    store::{self, Store},
};

/// A unit of work for [`Scheduler`]
pub struct Task {
    /// A reference to the [`query::Query`] which produced this work.
    pub query: query::Id,
    /// The [`Future`] that corresponds with this [`query::Query`]
    pub fut: Pin<Box<dyn Future<Output = store::Id> + Send>>,
    priority: usize,
}

#[derive(Default)]
struct Tasks {
    ready: BinaryHeap<Task>,
    blocked: HashMap<query::Id, Task>,
}

impl Tasks {
    fn wake(&mut self, id: &query::Id) {
        let task = self.blocked.remove(id).unwrap();
        self.ready.push(task);
    }
}

/// We only care about the priority when comparing two Tasks
impl PartialOrd for Task {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// We only care about the priority when comparing two Tasks
impl Ord for Task {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.priority.cmp(&other.priority)
    }
}

impl PartialEq for Task {
    fn eq(&self, other: &Self) -> bool {
        self.query == other.query
    }
}

impl Eq for Task {}

impl Task {
    /// Create a new instance with no set priority yet. Priority will be set by
    /// the [`Scheduler`].
    pub fn new(query: query::Id, ctx: &Global, priority: usize) -> Self {
        let actual = ctx.get_query(query).unwrap();
        let fut = actual.query(ctx.store());

        Self {
            query,
            fut,
            priority,
        }
    }
}

/// Bookkeeping to keep track of which tasks can be run next.
#[derive(Default)]
pub struct Scheduler<P: Policy> {
    tasks: Tasks,
    graph: Graph,
    _policy: PhantomData<P>,
}

impl<P: Policy> Scheduler<P> {
    pub fn new() -> Self {
        let tasks = Tasks::default();
        let graph = Graph::new();
        Self {
            tasks,
            graph,
            _policy: PhantomData::<P>,
        }
    }
    /// Submit a task to schedule. This is usually interfaced to by [`Ctx`]
    /// whenever someone requests a query, or by [`crate::runtime::Runtime`].
    /// This method assumes that the submitted task is not blocked by anything.
    pub(crate) fn score(&self, query: query::Id, ctx: &Global) -> usize {
        P::score(query, ctx)
    }

    pub(crate) fn submit(&mut self, task: Task) {
        self.tasks.ready.push(task);
    }

    /// Block until a runnable task exists.
    pub(crate) fn next(&self) -> Option<Task> {
        todo!()
    }

    /// The worker polled this task and got Pending, so remove it from the
    /// ready queue and move it to blocked.
    pub(crate) fn park(&mut self, task: Task) {
        // TODO: this is inefficient, see if this can be made faster
        self.tasks.ready.retain(|x| x.query != task.query);
        self.tasks.blocked.insert(task.query, task);
    }

    /// Something awaited by this task became ready, so move it to [`Self::ready`]
    /// This is basically the inverse of [`Self::park`]
    pub(crate) fn wake(&mut self, id: &query::Id) {
        let task = self.tasks.blocked.remove(id).unwrap();
        self.tasks.ready.push(task);
    }

    /// The task has been completed
    pub(crate) fn complete(&mut self, task: Task, output: store::Id) {
        for dependent in self.graph.dependents_of(task.query) {
            self.tasks.wake(dependent);
        }

        todo!("Return `output` to whichever requested it")
    }
}
