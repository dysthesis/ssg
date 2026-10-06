Task lifecycle
==============

This document briefly describes how tasks are created and executed.

Creation
--------

A `Task` is first created when `Ctx::run()` or `Ctx::query()` is called. The
difference is that the former is synchronous -- it is meant to run the root
task. The latter also requires the `query::Id` of the caller, in order to keep
track of dependencies.

Scheduling
----------

The two methods described above registers the new `Task` in `Scheduler`, which

- keeps track of which `Task` is ready, and which ones are blocked (_i.e._) have
  unsatisfied dependencies, and
- sorts the `ready` queue based on some scheduling `Policy`, whose goal is to
  prioritise `Task`s to minimise the idleness of the worker threads.

`Scheduler` itself does not know which other `Task` a `Task` depends on, only
that it does depend on something. When invoked, `Ctx::query()` also registers
an edge `from -> what` in `Graph` that keeps track of who depends on who. 
Then, when `what` is complete, `Scheduler` queries for the list of dependents 
and moves them to the ready queue.

Execution
---------

The `Scheduler` does not run the `Task`, it's only for bookkeeping. Instead,
the `Runtime` pulls a new `Task` from `Scheduler::next()` whenever a worker
thread is free, distributing it to the workers. When a worker is done with a
`Task`, which either means that it is

- blocked because that `Task` requested some dependency, or
- complete, returning the resulting `store::Id`,

it returns the results back to `Ctx`.

**Note.** We assume that `Query::query()` returns `Poll::Pending` only when
it calls `Ctx::query`.

Worker threads
--------------

A worker thread holds two channels, namely

- `work_rx`, an MPMC where it receives work from the `Runtime`, and
- `outcome_tx`, an MPSC where it sends the result when it can't progress
 further, _i.e._ the task becomes blocked or is complete.

It loops endlessly on `work_rx`, waiting on a work to do, and when a `Task`
arrrives, it

- gets the `Future` inside the `Task`,
- sets up a waker, which enqueues the `Task` back to the ready queue; this is
  necessary in case this `Task` requests some dependency and becomes blocked,
  in order to wake it back up when its dependency is done;
- polls the `Future`, and returns the result back to the `Runtime` through
  `outcome_tx`, without caring what the result is.
