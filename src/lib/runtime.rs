use std::{
    sync::{mpsc, Arc},
    task::{Context, Poll, Wake, Waker},
    thread::{self, JoinHandle, Thread},
};

use crate::{ctx::Ctx, query, scheduler::Task, store};

/// A "dumb" handle to a bunch of worker threads that distributes work to them
/// and reports back results
pub struct Runtime<const N: usize> {
    work_tx: crossbeam_channel::Sender<Task>,
    event_rx: mpsc::Receiver<Event>,
    pool: [JoinHandle<()>; N],
}

/// The runtime doesn't really care what the "real" result is, it only cares if
/// it needs to wake up some other query.
pub enum Event {
    /// No need to wake up anything yet
    Polled(Task, Poll<store::Id>),
    /// Wake up this query
    Wake(query::Id),
}

impl<const N: usize> Runtime<N> {
    pub fn init() -> Self {
        let (work_tx, work_rx) = crossbeam_channel::unbounded::<Task>();
        let (event_tx, event_rx) = mpsc::channel();

        let pool = std::array::from_fn(|_| {
            let work_rx = work_rx.clone();
            let event_tx = event_tx.clone();

            thread::spawn(move || {
                while let Ok(mut task) = work_rx.recv() {
                    let waker = Waker::from(Arc::new(Reschedule {
                        event_tx: event_tx.clone(),
                        query: task.query,
                    }));

                    let mut cx = Context::from_waker(&waker);
                    let poll = task.fut.as_mut().poll(&mut cx);

                    if event_tx.send(Event::Polled(task, poll)).is_err() {
                        break;
                    }
                }
            })
        });

        Self {
            work_tx,
            event_rx,
            pool,
        }
    }

    /// Submit a new task for the [`Runtime`] to work on.
    #[inline]
    pub fn submit(&self, task: Task) {
        self.work_tx.send(task).unwrap();
    }

    /// Receive an [`Event`] that was sent by a worker thread.
    #[inline]
    pub fn recv(&self) -> Event {
        self.event_rx.recv().unwrap()
    }
}

struct Reschedule {
    event_tx: mpsc::Sender<Event>,
    query: query::Id,
}

impl Wake for Reschedule {
    fn wake(self: Arc<Self>) {
        let _ = self.event_tx.send(Event::Wake(self.query));
    }

    fn wake_by_ref(self: &Arc<Self>) {
        let _ = self.event_tx.send(Event::Wake(self.query));
    }
}
