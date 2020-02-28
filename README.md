# Executor Benchmarks

This repository holds some benchmarks for async executors in Rust. Currently there is but one benchmark, trying to evaluate the
overhead introduced by the different spawning traits provided by the [async_executors](https://crates.io/crates/async_executors) crate.
This branch holds the benchmarks for async_executors version 0.2. See the [master branch](https://github.com/najamelan/executor_benchmarks) for other versions.

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
- rustc: rustc 1.43.0-nightly (6fd8798f4 2020-02-25) (codegen-units=1)
- futures: 0.3.4
- tokio: 0.2.12
- async-std: 1.5.0

```
Executor                          spawn API               N   avg time/iter
----------------------------------------------------------------------------
LocalPool                         LocalSpawn             10   31.360 us
LocalPool                         LocalSpawnHandle       10   53.154 us

tokio::Runtime (LocalSet)         native                 10   33.328 us
TokioCt                           LocalSpawn             10   34.737 us
TokioCt                           LocalSpawnHandle       10   53.672 us

ThreadPool                        Spawn                  10   233.73 us
ThreadPool                        SpawnHandle            10   276.03 us

tokio::Runtime                    native                 10   95.656 us
TokioTp                           Spawn                  10   92.409 us
TokioTp                           SpawnHandle            10   104.40 us

async_std::task                   native                 10   101.35 us
AsyncStd                          Spawn                  10   121.77 us
AsyncStd                          SpawnHandle            10   106.85 us

LocalPool                         LocalSpawn            100   3.1649 ms
LocalPool                         LocalSpawnHandle      100   5.1421 ms

tokio::Runtime (LocalSet)         native                100   3.0230 ms
TokioCt                           LocalSpawn            100   3.0517 ms
TokioCt                           LocalSpawnHandle      100   5.2575 ms

ThreadPool                        Spawn                 100   14.940 ms
ThreadPool                        SpawnHandle           100   15.041 ms

tokio::Runtime                    native                100   1.8322 ms
TokioTp                           Spawn                 100   1.6033 ms
TokioTp                           SpawnHandle           100   1.9112 ms

async_std::task                   native                100   785.92 us
AsyncStd                          Spawn                 100   950.68 us
AsyncStd                          SpawnHandle           100   905.42 us

LocalPool                         LocalSpawn            200   12.334 ms
LocalPool                         LocalSpawnHandle      200   20.535 ms

tokio::Runtime (LocalSet)         native                200   11.850 ms
TokioCt                           LocalSpawn            200   11.743 ms
TokioCt                           LocalSpawnHandle      200   24.448 ms

ThreadPool                        Spawn                 200   58.284 ms
ThreadPool                        SpawnHandle           200   59.411 ms

tokio::Runtime                    native                200   4.8219 ms
TokioTp                           Spawn                 200   3.6299 ms
TokioTp                           SpawnHandle           200   4.9988 ms

async_std::task                   native                200   2.2333 ms
AsyncStd                          Spawn                 200   2.4586 ms
AsyncStd                          SpawnHandle           200   2.6674 ms
```

### Breakdown

#### Differences from 0.1

We now use `tokio::task::LocalSet` for spawning `!Send` futures. This massively improves the performance.
The non object safe `SpawnHandle` traits have been removed since the 0.1 benchmarks have shown that
boxing was really cheap.

We'll be looking at the N=200 benchmarks here, because I think these have the best signal/noise ratio.
These benchmarks spawn about 40,200 tasks each.

The different spawn API's compared here:

- native: _tokio_ or _async-std_ without any code from _async_executors_.

- `Spawn`/`LocalSpawn`: The traits from _futures_, implemented on a wrapper in _async_executors_. Note that these
  traits take `FutureObj`, which is a `no_std` compatible equivalent to `Box<dyn Future>`, so we'd expect to
  see some overhead compared to native.

- `SpawnHandle`/`LocalSpawnHandle`: Traits from _async_executors_ that return a `JoinHandle`. This `JoinHandle` is
  executor agnostic and wraps the native JoinHandle types from executors that provide one. The traits are
  implemented on the _futures_ executors, where `futures_util::future::RemoteHandle` is used.
  In order to make a consistent and feature full API, we wrap the futures in `futures_util::future::Abortable`
  on both _tokio_ and _async_std_, so it's possible to both cancel a running future and detach from it.
  It's expected to see some overhead from `Abortable`. These traits also take a `FutureObj` so an extra allocation takes place.
  The _futures_ executors get the worst of it with the overhead from `RemoteHandle`. `TokioCt` also
  pays for `RemoteHandle` since the native `JoinHandle` does not support an output type that is `!Send`.

### Conclusions

We can see that the total overhead going from native to SpawnHandle lays between 4ns and 10ns per spawn. About
19.4% for async-std and 3.6% for TokioTp. This includes the price of extra boxing as well as using `AbortHandle`
to enable both detaching and dropping a future from the JoinHandle. I feel the added interoperability is well worth the price.

Where the use of RemoteHandle is required (futures executors and TokioCt), the price is more like 30ns per spawn.

There is a large gap in spawn overhead between _tokio_ and _async_std_, with _async_std_ being up to twice as fast,
but don't be fooled, this benchmark is not meant to make a meaningful comparison between the two and it's not a good
test for actual scheduler implementations. Spawning overhead will be a neglectable factor for almost all asynchronous
applications.

The big difference between the two is that _tokio_ let's you control a self contained `Runtime` which is configurable,
where _async_std_ only provides a global spawn function.
