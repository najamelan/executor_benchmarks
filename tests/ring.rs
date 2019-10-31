
use
{
	executor_benchmarks :: * ,
	async_executors     :: * ,
	futures             :: { task::LocalSpawnExt, executor::block_on } ,
	log                 :: * ,
};


#[test]
//
fn basic()
{
	let _ = flexi_logger::Logger::with_str( "warn, executor_benchmarks=trace" ).start();

	let mut pool    = LocalPool::new();
	let mut spawner = pool.handle();

	let bench = async move
	{
		let mut ring = Ring::new( 3 );
		ring.run( &mut spawner ).await;
	};

	pool.spawn_local( bench ).expect( "spawn bench" );

	pool.run();
}


#[test]
//
fn threadpool()
{
	let _ = flexi_logger::Logger::with_str( "warn, executor_benchmarks=trace" ).start();

	for i in 1..2
	{
		warn!( "Test: starting run {}", i );

		let mut pool = ThreadPool::new().expect( "create threadpool" );
		let mut ring = Ring::new( 2 );

		block_on( ring.run( &mut pool ) );

		warn!( "Test: ending run {}", i );
	}
}

