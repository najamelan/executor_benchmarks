//! This uses bounded channels and has the possibility to spawn each message forwarding on the executor
//! rather than
use futures::{ SinkExt, StreamExt, channel::mpsc, task::{ LocalSpawn, LocalSpawnExt, Spawn, SpawnExt } };
use std::{ sync::{ atomic::{ AtomicUsize, Ordering } } };
use log::*;

static DONE: AtomicUsize = AtomicUsize::new( 0 );


pub struct BoundedRing
{
	nodes: Option< Vec<BoundedNode> > ,
}


impl BoundedRing
{
	// Create channels between all the nodes.
	//
	pub fn new( n: usize ) -> Self
	{
		assert!( n > 1 );

		let mut nodes = Vec::with_capacity( n );

		// The channel always has one slot per sender, so this is a bounded
		// channel of size 1. I choose a low number in the hopes that some
		// tasks will block, creating a bit more work for the executor and
		// a more realistic benchmark.
		//
		let channel_size = 0;

		// The connection between the last and the first node.
		//
		let (last_tx, last_rx) = mpsc::channel( channel_size );

		// The first node
		//
		let (tx, mut next_rx) = mpsc::channel( channel_size );

		nodes.push( BoundedNode { id: 1, n, tx, rx: last_rx } );


		// All but first and last.
		// Note that 1..1 does not do anything, so it works if n==2.
		//
		for id in 2..n
		{
			let (tx, rx) = mpsc::channel( channel_size );

			nodes.push( BoundedNode { id, n, tx, rx: next_rx } );

			next_rx = rx;
		}


		// The last node
		//
		nodes.push( BoundedNode { id: n, n, tx: last_tx, rx: next_rx } );

		Self
		{
			nodes: Some( nodes ),
		}
	}



	// // Return a vec of futures that await the operation of each node.
	// // You can either spawn them, join them, use futures unordered, ... to compare performance.
	// //
	// pub fn start_parallel( &mut self ) -> Vec<impl Future<Output = ()> >
	// {
	// 	self.nodes.take().unwrap().into_iter().map( move |mut node|
	// 	{
	// 		async move
	// 		{
	// 			node.run().await;
	// 		}

	// 	}).collect()
	// }



	// Run the benchmark.
	//
	pub async fn run( &mut self, mut exec: impl Spawn + Clone + Send + 'static )
	{
		debug!( "BoundedRing: start" );

		// Need to reset the DONE counter, since it might be reused on repeated benchmarks.
		//
		DONE.store( 0, Ordering::SeqCst );


		let (done_tx, mut done_rx) = mpsc::channel(0);


		for mut node in self.nodes.take().unwrap().into_iter()
		{
			let done_tx = done_tx.clone();

			let exec2 = exec.clone();
			exec.spawn( async move { node.run( done_tx, exec2 ).await; } ).expect( "spawn node" );
		};

		let res = done_rx.next().await;

		debug_assert!( res.is_some() );

		debug!( "BoundedRing: end" );


		// Just a memory barrier.
		//
		// DONE.store( 0, Ordering::SeqCst );
	}



	// Run the benchmark on a local pool.
	//
	pub async fn run_local( &mut self, mut exec: impl LocalSpawn + Spawn + Clone + 'static )
	{
		debug!( "BoundedRing: start" );

		// Need to reset the DONE counter, since it might be reused on repeated benchmarks.
		//
		DONE.store( 0, Ordering::SeqCst );


		let (done_tx, mut done_rx) = mpsc::channel(0);


		for mut node in self.nodes.take().unwrap().into_iter()
		{
			let done_tx = done_tx.clone();

			let exec2 = exec.clone();
			exec.spawn_local( async move { node.run( done_tx, exec2 ).await; } ).expect( "spawn node" );
		};

		let res = done_rx.next().await;

		debug_assert!( res.is_some() );

		debug!( "BoundedRing: end" );


		// Just a memory barrier.
		//
		// DONE.store( 0, Ordering::SeqCst );
	}
}


// Each node will start by sending a 1 to the next node. When the counter has come back and
// been incremented by all nodes, the operation is complete.
//
pub struct BoundedNode
{
	id: usize                 ,
	n : usize                 ,
	tx: mpsc::Sender  <usize> ,
	rx: mpsc::Receiver<usize> ,
}

impl BoundedNode
{
	async fn run( &mut self, mut done_tx: mpsc::Sender<()>, mut exec: impl Spawn )
	{
		debug!( "BoundedNode {}: run", self.id );

		let mut tx = self.tx.clone();
		exec.spawn( async move { tx.send( 1 ).await.expect( "Node: send initial message" ); } )

			.expect( "spawn forward" );


		// Two ways out of this loop:
		// - if we are the last node to finish, we break and then close our channel.
		// - if we aren't the last, we will end the loop when the channel is closed and returns None.
		//
		while let Some(msg) = self.rx.next().await
		{
			debug_assert!( !msg > self.n );

			// When our message comes back, it should be counted by everyone, so it should be n.
			// if the msg < n, it didn't originate here.
			//
			if msg == self.n
			{
				trace!( "BoundedNode {}: received our own message back", self.id );

				// Store the fact that we are done.
				//
				let old = DONE.fetch_add( 1, Ordering::SeqCst );

				// If we are the last one.
				//
				if old+1 == self.n
				{
					trace!( "BoundedNode {}: all done", self.id );

					done_tx.send(()).await.expect( "Send DONE" );

					break;
				}

				// else continue passing on messages for the other nodes.
				//
				continue;
			}

			trace!( "BoundedNode {}: forwarding a message", self.id );

			// forward the message
			//
			let mut tx = self.tx.clone();

			exec.spawn( async move { tx.send( msg + 1 ).await.expect( "BoundedNode: forward message" ); } )

				.expect( "spawn forward" );
		}

		trace!( "BoundedNode {}: close our sender", self.id );

		// Allow nodes that where still passing on nodes to detect that we are done.
		//
		self.tx.close().await.expect( "close channel" );

		debug!( "BoundedNode {}: run END", self.id );
	}
}



#[ cfg( test ) ]
//
mod tests
{
	#[ allow( unused_imports ) ] // false positive
	//
	use super::*;

	#[test]
	//
	fn off_by_one_or_not()
	{
		let ring2 = BoundedRing::new( 2 );
		assert_eq!( 2, ring2.nodes.unwrap().len() );

		let ring2 = BoundedRing::new( 3 );
		assert_eq!( 3, ring2.nodes.unwrap().len() );
	}
}
