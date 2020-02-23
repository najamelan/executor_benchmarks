# Executor Benchmarks

This repository holds some benchmarks for async executors in Rust. Currently there is but one benchmark, trying to evaluate the
overhead introduced by the different spawning traits provided by the [async_executors](https://crates.io/crates/async_executors) crate.

## Ring: concurrent message passing

Create N nodes in a ring. Let every node send a usize counter to the next until the message has come back to the initiating node. Each node should initiate, so that in total N^2 messages get send. Each node compares the counter. If it is N, they stop, if counter < N, they increment the counter and pass the message to the next node, so every message makes an entire round around the ring.

### Implementation

Each node is spawned as a task, as well as each send on a channel. We get about N^2+N spawns. Some extra synchronization is needed in certain
cases to avoid nodes from outliving the benchmark iteration. As expected awaiting JoinHandles greatly simplifies this compared
to spawning orphaned tasks.

### Disclaimer

- Writing correct and meaningful benchmarks isn't easy, and this hasn't been reviewed by anyone
- It's a micro-benchmark, there isn't any IO in this, which would be the most common application of async rust. We are only spawning a lot and trying to get an idea of the overhead of different spawning API's.
- Spawning overhead is not important for 99.99% of asynchronous applications, and this benchmark does not cover common executor usage, don't base your choice of executor on this benchmark.
- Local vs Threadpool results might be skewed because the tasks don't do any work other than sending on a channel. Thus the benefit you could get from multi-threading is underestimated in this benchmark, and probably overshadowed by the thread synchronization in the channels.
- I have not done any profiling to verify where time gets spent to have a real understanding of the performance profile.
- Results can be skewed by [binary layout and other shenanigans](https://www.youtube.com/watch?v=r-TLSBdHe1A).
- There are no benchmarks for wasm-bindgen-futures


### Results

The following results are from running `cargo bench --bench ring` on a machine with the following specs:

- CPU: AMD® Ryzen 9 3900x 12-core processor × 24
- Memory: GSKILL DDR4 32GB (4x8) F4-3600C18Q-32GVK
- MB: Gigabyte X570 Aorus Elite

```
Executor                          spawn API               N   avg time/iter
----------------------------------------------------------------------------
LocalPool                         LocalSpawn             10   32.607 us
LocalPool                         LocalSpawnHandle       10   54.327 us
LocalPool                         LocalSpawnHandleOs     10   55.195 us

TokioCt                           LocalSpawn             10   227.04 us
TokioCt                           LocalSpawnHandle       10   260.56 us
TokioCt                           LocalSpawnHandleOs     10   268.46 us

ThreadPool                        Spawn                  10   228.94 us
ThreadPool                        SpawnHandle            10   268.12 us
ThreadPool                        SpawnHandleOs          10   271.40 us

tokio::Runtime                    native                 10   88.444 us
tokio::Runtime (basic_scheduler)  native                 10   238.02 us
TokioTp                           Spawn                  10   85.854 us
TokioTp                           SpawnHandle            10   94.428 us
TokioTp                           SpawnHandleOs          10   95.026 us

async_std::task                   native                 10   70.200 us
AsyncStd                          Spawn                  10   93.593 us
AsyncStd                          SpawnHandle            10   84.071 us
AsyncStd                          SpawnHandleOs          10   75.715 us

LocalPool                         LocalSpawn            100   3.2316 ms
LocalPool                         LocalSpawnHandle      100   5.0745 ms
LocalPool                         LocalSpawnHandleOs    100   5.1460 ms

TokioCt                           LocalSpawn            100   23.873 ms
TokioCt                           LocalSpawnHandle      100   28.017 ms
TokioCt                           LocalSpawnHandleOs    100   28.402 ms

ThreadPool                        Spawn                 100   15.105 ms
ThreadPool                        SpawnHandle           100   15.320 ms
ThreadPool                        SpawnHandleOs         100   15.154 ms

tokio::Runtime                    native                100   1.7972 ms
tokio::Runtime (basic_scheduler)  native                100   25.904 ms
TokioTp                           Spawn                 100   1.5723 ms
TokioTp                           SpawnHandle           100   1.9379 ms
TokioTp                           SpawnHandleOs         100   1.8808 ms

async_std::task                   native                100   855.92 us
AsyncStd                          Spawn                 100   1.0351 ms
AsyncStd                          SpawnHandle           100   937.66 us
AsyncStd                          SpawnHandleOs         100   964.51 us

LocalPool                         LocalSpawn            200   12.919 ms
LocalPool                         LocalSpawnHandle      200   20.120 ms
LocalPool                         LocalSpawnHandleOs    200   20.615 ms

TokioCt                           LocalSpawn            200   94.493 ms
TokioCt                           LocalSpawnHandle      200   116.43 ms
TokioCt                           LocalSpawnHandleOs    200   118.19 ms

ThreadPool                        Spawn                 200   57.790 ms
ThreadPool                        SpawnHandle           200   58.886 ms
ThreadPool                        SpawnHandleOs         200   57.828 ms

tokio::Runtime                    native                200   4.7856 ms
tokio::Runtime (basic_scheduler)  native                200   108.65 ms
TokioTp                           Spawn                 200   3.5806 ms
TokioTp                           SpawnHandle           200   5.0798 ms
TokioTp                           SpawnHandleOs         200   5.1429 ms

async_std::task                   native                200   2.4121 ms
AsyncStd                          Spawn                 200   2.6799 ms
AsyncStd                          SpawnHandle           200   2.7550 ms
AsyncStd                          SpawnHandleOs         200   2.8141 ms
```

### Breakdown

We'll be looking at the N=200 benchmarks here, because I think these have the best signal/noise ratio.
These benchmarks spawn about 40,200 tasks each.

The different spawn API's compared here:

- native: _tokio_ or _async-std_ without any code from _async_executors_.

- `Spawn`/`LocalSpawn`: The traits from _futures_, implemented on a wrapper in _async_executors_. Note that these
  traits take `FutureObj`, which is a `no_std` compatible equivalent to `Box<dyn Future>`, so we'd expect to
  see some overhead compared to native.
  _async_executors_ also provides `LocalSpawn` for the `tokio` runtime with basic scheduler, even though out of the
  box this Runtime does not provide an API for spawning `!Send` futures. Therefor there is no `native` version to compare.

- `SpawnHandle`/`LocalSpawnHandle`: Traits from _async_executors_ that return a `JoinHandle`. This `JoinHandle` is
  executor agnostic and wraps the native JoinHandle types from executors that provide one. The traits are
  implemented on the _futures_ executors, where `futures_util::future::RemoteHandle` is used.
  In order to make a consistent and feature full API, we wrap the futures in `futures_util::future::Abortable`
  on both _tokio_ and _async_std_, so it's possible to both cancel a running future and detach from it.
  It's expected to see some overhead from `Abortable`, however we no longer need to box the futures as with `Spawn`/`LocalSpawn`.
  The _futures_ executors get the worst of it with both boxing and the overhead from `RemoteHandle`. `TokioCt` also
  pays for `RemoteHandle` since the native `JoinHandle` does not support an output type that is `!Send`.

- `SpawnHandleOs`/`SpawnHandleLocalOs`: The disadvantage of the previous traits is that they are not object safe.
  This is severely limiting for API's that need to store an executor for usage over some time. The price for object
  safety here is that the trait needs to be generic over the `Future::Output` associated type and that we need to
  box the future again. So we look at the benchmark to see how much this boxing actually costs.

### Conclusions

The benchmarks teach us that boxing, both in `Spawn` and in `SpawnHandleOs` is particularly cheap. We get about
1.5ns/spawn overhead. The benchmark isn't even precise enough to measure this, as sometimes `SpawnHandleOs` shows
faster results than `SpawnHandle`. Most applications will not have to spawn 40k tasks in a short period of time, so this is
neglectable in my opinion.

Adding extra functionality around futures with `Abortable` in `SpawnHandle` is more costly. Going from the native to
`SpawnHandle` benchmarks we get between 4ns and 8ns per spawn.

I think the extra benefits of interoperability and consistent API outweigh the performance cost by far for any practical
application.

There is a large gap in spawn overhead between _tokio_ and _async_std_, with _async_std_ being up to twice as fast,
but don't be fooled, this benchmark is not meant to make a meaningful comparison between the two and it's not a good
test for actual scheduler implementations. Spawning overhead will be a neglectable factor for almost all asynchronous
applications.

The big difference between the two is that _tokio_ let's you control a self contained `Runtime` which is configurable,
where _async_std_ only provides a global spawn function.
