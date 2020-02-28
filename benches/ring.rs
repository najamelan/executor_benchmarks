use
{
	executor_benchmarks :: * ,
	criterion           :: { Criterion, criterion_group, criterion_main } ,
	async_executors     :: { *                                          } ,
	tokio               :: { runtime::Builder, task::LocalSet           } ,
	std                 :: { convert::TryFrom                           } ,
	futures::executor   :: { LocalPool, ThreadPool, block_on            } ,
};



fn ring( c: &mut Criterion )
{
	// let _ = flexi_logger::Logger::with_str( "warn, executor_benchmarks=trace" ).start();


	let mut group = c.benchmark_group( "Ring" );

	for nodes in [10, 100, 200].iter()
	{
		match nodes
		{
			10  => { group.sample_size( 100 ); }
			100 => { group.sample_size( 50  ); }
			200 => { group.sample_size( 30  ); }
			_   => { unreachable!();           }
		}


		group.bench_function( format!( "LocalPool spawn_local {}", &nodes ), |b|
		{
			let mut pool     = LocalPool::new();
			let     spawner  = pool.spawner();

			b.iter( ||
			{
				let mut ring = LocalRing::new( *nodes );

				pool.run_until( ring.run( spawner.clone() ) );
			});
		});


		group.bench_function( format!( "LocalPool spawn_handle_local {}", &nodes ), |b|
		{
			let mut pool     = LocalPool::new();
			let     spawner  = pool.spawner();

			b.iter( ||
			{
				let mut ring = LocalHandleRing::new( *nodes );

				pool.run_until( ring.run( spawner.clone() ) );
			});
		});


		group.bench_function( format!( "tokio::runtime::Runtime (basic_scheduler + LocalSet) {}", &nodes ), |b|
		{
			let mut pool = Builder::new().basic_scheduler().build().expect( "build tokio threadpool" );

			b.iter( ||
			{
				let exec = LocalSet::new();
				let mut ring = TokioCtNativeRing::new( *nodes );

				exec.spawn_local( async move { ring.run().await; } );
				pool.block_on( exec );
			});
		});


		group.bench_function( format!( "TokioCt spawn_local {}", &nodes ), |b|
		{
			let     pool  = TokioCt::try_from( &mut Builder::new() ).expect( "build tokio basic_scheduler" );
			let mut pool2 = pool.clone();

			b.iter( ||
			{
				let pool3 = pool.clone();

				let bench = async move
				{
					let mut ring = LocalRing::new( *nodes );
					ring.run( pool3 ).await;
				};

				pool2.block_on( bench );
			});
		});


		group.bench_function( format!( "TokioCt spawn_handle_local {}", &nodes ), |b|
		{
			let     pool  = TokioCt::try_from( &mut Builder::new() ).expect( "build tokio basic_scheduler" );
			let mut pool2 = pool.clone();

			b.iter( ||
			{
				let pool3 = pool.clone();

				let bench = async move
				{
					let mut ring = LocalHandleRing::new( *nodes );
					ring.run( pool3 ).await;
				};

				pool2.block_on( bench );
			});
		});


		group.bench_function( format!( "ThreadPool spawn {}", &nodes ), |b|
		{
			let pool = ThreadPool::new().expect( "create threadpool" );

			b.iter( move ||
			{
				let pool2 = pool.clone();

				let mut ring = BoundedRing::new( *nodes );

				block_on( ring.run( pool2 ) );
			});
		});


		group.bench_function( format!( "ThreadPool spawn_handle {}", &nodes ), |b|
		{
			let pool = ThreadPool::new().expect( "create threadpool" );

			b.iter( move ||
			{
				let pool2 = pool.clone();

				let mut ring = HandleRing::new( *nodes );

				block_on( ring.run( pool2 ) );
			});
		});


		group.bench_function( format!( "tokio::runtime::Runtime {}", &nodes ), |b|
		{
			let mut pool = Builder::new().threaded_scheduler().build().expect( "build tokio threadpool" );

			b.iter( ||
			{
				let mut ring = TokioTpNativeRing::new( *nodes );

				pool.block_on( ring.run( pool.handle().clone() ) );
			});
		});


		group.bench_function( format!( "TokioTp Spawn {}", &nodes ), |b|
		{
			let mut pool = TokioTp::try_from( &mut Builder::new() ).expect( "build tokio threadpool" );

			b.iter( ||
			{
				let mut ring = BoundedRing::new( *nodes );

				pool.block_on( ring.run( pool.clone() ) );
			});
		});


		group.bench_function( format!( "TokioTp SpawnHandle {}", &nodes ), |b|
		{
			let mut pool = TokioTp::try_from( &mut Builder::new() ).expect( "build tokio threadpool" );

			b.iter( ||
			{
				let mut ring = HandleRing::new( *nodes );

				pool.block_on( ring.run( pool.clone() ) );
			});
		});


		group.bench_function( format!( "async_std::task::spawn {}", &nodes ), |b|
		{
			b.iter( ||
			{
				let mut ring = AsyncStdNativeRing::new( *nodes );

				async_std::task::block_on( ring.run() );
			});
		});


		group.bench_function( format!( "AsyncStd Spawn {}", &nodes ), |b|
		{
			let pool = AsyncStd::default();

			b.iter( ||
			{
				let mut ring = BoundedRing::new( *nodes );

				AsyncStd::block_on( ring.run( pool ) );
			});
		});


		group.bench_function( format!( "AsyncStd SpawnHandle {}", &nodes ), |b|
		{
			let pool = AsyncStd::default();

			b.iter( ||
			{
				let mut ring = HandleRing::new( *nodes );

				AsyncStd::block_on( ring.run( pool ) );
			});
		});
	}
}

criterion_group!(benches, ring);
criterion_main! (benches);
