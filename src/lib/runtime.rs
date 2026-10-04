use std::thread::JoinHandle;

use crossbeam_channel::Sender;

use crate::scheduler::Task;

pub struct Runtime<const N: usize> {
    tx: Sender<Task>,
    pool: [JoinHandle<()>; N],
}

impl<const N: usize> Runtime<N> {
    pub fn init() -> Self {
        todo!()
    }
}
