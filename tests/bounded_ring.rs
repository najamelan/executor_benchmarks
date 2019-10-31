
use
{
	executor_benchmarks :: * ,
	async_executors     :: * ,
	futures             :: { executor::block_on } ,
	log                 :: * ,
};



#[test]
//
fn threadpool()
{
	let _ = flexi_logger::Logger::with_str( "warn, executor_benchmarks=trace" ).start();

	for i in 1..2
	{
		warn!( "Test: starting run {}", i );

		let pool = ThreadPool::new().expect( "create threadpool" );
		let mut ring = BoundedRing::new( 2 );

		block_on( ring.run( pool ) );

		warn!( "Test: ending run {}", i );
	}
}

