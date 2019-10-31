use
{
	executor_benchmarks :: * ,
	criterion           :: { Criterion, criterion_group, criterion_main } ,
	futures::executor   :: { block_on } ,
	futures::task       :: { LocalSpawnExt } ,
	async_executors     :: { * } ,
};



fn ring( c: &mut Criterion )
{
	// let _ = flexi_logger::Logger::with_str( "warn, executor_benchmarks=trace" ).start();


	let mut group = c.benchmark_group( "BoundedRing benchmark" );

	for nodes in [3, 10, 100].iter()
	{
		group.bench_function( format!( "LocalPool spawn {}", &nodes ), |b|
		{
			b.iter( ||
			{
				let mut pool    = LocalPool::new();
				let     spawner = pool.handle();

				let bench = async move
				{
					let mut ring = BoundedRing::new( *nodes );
					ring.run_local( spawner ).await;
				};

				pool.spawn_local( bench ).expect( "spawn bench" );

				pool.run();
			});
		});


		group.bench_function( format!( "TokioCt spawn {}", &nodes ), |b|
		{
			b.iter( ||
			{
				let mut pool    = TokioCt::new();
				let     spawner = pool.handle();

				let bench = async move
				{
					let mut ring = BoundedRing::new( *nodes );
					ring.run_local( spawner ).await;
				};

				pool.spawn_local( bench ).expect( "spawn bench" );

				pool.run().expect( "run tokio_ct" );
			});
		});


		group.bench_function( format!( "ThreadPool spawn {}", &nodes ), |b|
		{
			b.iter( ||
			{
				let     pool = ThreadPool::new().expect( "create threadpool" );
				let mut ring = BoundedRing::new( *nodes );

				block_on( ring.run( pool ) );
			});
		});


		group.bench_function( format!( "TokioTp spawn {}", &nodes ), |b|
		{
			b.iter( ||
			{
				let     pool = TokioTp::new();
				let mut ring = BoundedRing::new( *nodes );

				block_on( ring.run( pool.handle() ) );
			});
		});


		group.bench_function( format!( "Juliex spawn {}", &nodes ), |b|
		{
			b.iter( ||
			{
				let     pool = Juliex::new();
				let mut ring = BoundedRing::new( *nodes );

				block_on( ring.run( pool ) );
			});
		});


		group.bench_function( format!( "AsyncStd spawn {}", &nodes ), |b|
		{
			b.iter( ||
			{
				let     pool = AsyncStd::new();
				let mut ring = BoundedRing::new( *nodes );

				block_on( ring.run( pool ) );
			});
		});
	}
}

criterion_group!(benches, ring);
criterion_main! (benches);
