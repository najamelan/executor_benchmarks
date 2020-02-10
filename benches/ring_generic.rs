use
{
	executor_benchmarks :: * ,
	criterion           :: { Criterion, criterion_group, criterion_main } ,
	futures::executor   :: { block_on, LocalPool, ThreadPool            } ,
	futures::task       :: { LocalSpawnExt                              } ,
	async_executors     :: { *                                          } ,
	tokio               :: { runtime::Builder                           } ,
	std                 :: { convert::TryFrom                           } ,
};



fn ring( c: &mut Criterion )
{
	// let _ = flexi_logger::Logger::with_str( "warn, executor_benchmarks=trace" ).start();


	let mut group = c.benchmark_group( "Ring benchmark" );

	for nodes in [3, 10, 100].iter()
	{
		group.bench_function( format!( "LocalPool spawn {}", &nodes ), |b|
		{
			b.iter( ||
			{
				let mut pool    = LocalPool::new();
				let spawner = pool.spawner();
				let mut spawner2 = spawner.clone();

				let bench = async move
				{
					let mut ring = Ring::new( *nodes );
					ring.run( &mut spawner2 ).await;
				};

				spawner.spawn_local( bench ).expect( "spawn bench" );

				pool.run();
			});
		});


		// group.bench_function( format!( "TokioCt spawn {}", &nodes ), |b|
		// {
		// 	b.iter( ||
		// 	{
		// 		let mut pool    = TokioCt::new();
		// 		let mut spawner = pool.handle();

		// 		let bench = async move
		// 		{
		// 			let mut ring = Ring::new( *nodes );
		// 			ring.run( &mut spawner ).await;
		// 		};

		// 		pool.spawn_local( bench ).expect( "spawn bench" );

		// 		pool.run().expect( "run tokio_ct" );
		// 	});
		// });


		group.bench_function( format!( "ThreadPool spawn {}", &nodes ), |b|
		{
			b.iter( ||
			{
				let mut pool = ThreadPool::new().expect( "create threadpool" );
				let mut ring = Ring::new( *nodes );

				block_on( ring.run( &mut pool ) );
			});
		});


		group.bench_function( format!( "TokioTp spawn {}", &nodes ), |b|
		{
			b.iter( ||
			{
				let mut pool = TokioTp::try_from( &mut Builder::new() ).expect( "build tokio threadpool" );
				let mut spawner  = pool.handle();
				let mut ring = Ring::new( *nodes );

				pool.block_on( ring.run( &mut spawner ) );
			});
		});


		group.bench_function( format!( "AsyncStd spawn {}", &nodes ), |b|
		{
			b.iter( ||
			{
				let mut pool = AsyncStd::default();
				let mut ring = Ring::new( *nodes );

				block_on( ring.run( &mut pool ) );
			});
		});
	}
}

criterion_group!(benches, ring);
criterion_main! (benches);
