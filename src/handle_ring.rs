//! This uses bounded channels and has the possibility to spawn each message forwarding on the executor
//! rather than awaiting.
//!
use futures::{ future::join_all, SinkExt, StreamExt, channel::mpsc };
use std::{ sync::{ atomic::{ AtomicUsize, Ordering } } };
use log::*;
use async_executors::*;

static DONE: AtomicUsize = AtomicUsize::new( 0 );


pub struct HandleRing
{
	nodes: Option< Vec<HandleNode> > ,
}


impl HandleRing
{
	// Create channels between all the nodes.
	//
	pub fn new( n: usize ) -> Self
	{
		assert!( n > 1 );

		let mut nodes = Vec::with_capacity( n );

		// The channel always has one slot per sender, so this is a bounded
		// channel of size channel_size + 1. I choose a low number in the hopes
		// that some tasks will block, creating a bit more work for the executor
		// and a more realistic benchmark.
		//
		let channel_size = 1;

		// The connection between the last and the first node.
		//
		let (last_tx, last_rx) = mpsc::channel( channel_size );

		// The first node
		//
		let (tx, mut next_rx) = mpsc::channel( channel_size );

		nodes.push( HandleNode { id: 1, n, tx, rx: last_rx } );


		// All but first and last.
		// Note that 1..1 does not do anything, so it works if n==2.
		//
		for id in 2..n
		{
			let (tx, rx) = mpsc::channel( channel_size );

			nodes.push( HandleNode { id, n, tx, rx: next_rx } );

			next_rx = rx;
		}


		// The last node
		//
		nodes.push( HandleNode { id: n, n, tx: last_tx, rx: next_rx } );

		Self
		{
			nodes: Some( nodes ),
		}
	}


	// Run the benchmark.
	//
	pub async fn run( &mut self, exec: impl SpawnHandle<()> + Clone + Send + Sync + 'static )
	{
		debug!( "HandleRing: start" );

		// Need to reset the DONE counter, since it might be reused on repeated benchmarks.
		//
		DONE.store( 0, Ordering::SeqCst );


		let mut handles = Vec::with_capacity( self.nodes.as_ref().unwrap().len() );

		for mut node in self.nodes.take().unwrap().into_iter()
		{
			let exec2 = exec.clone();
			handles.push( exec.spawn_handle( async move { node.run( exec2 ).await; } ).expect( "spawn node" ) );
		};

		join_all( handles ).await;

		debug!( "HandleRing: end" );
	}



	// // Run the benchmark on a local pool.
	// //
	// pub async fn run_local( &mut self, exec: impl LocalSpawn + Spawn + Clone + 'static )
	// {
	// 	debug!( "HandleRing: start" );

	// 	// Need to reset the DONE counter, since it might be reused on repeated benchmarks.
	// 	//
	// 	DONE.store( 0, Ordering::SeqCst );


	// 	let (done_tx, mut done_rx) = mpsc::channel(0);


	// 	for mut node in self.nodes.take().unwrap().into_iter()
	// 	{
	// 		let done_tx = done_tx.clone();

	// 		let exec2 = exec.clone();
	// 		exec.spawn_local( async move { node.run( done_tx, exec2 ).await; } ).expect( "spawn node" );
	// 	};

	// 	let res = done_rx.next().await;

	// 	debug_assert!( res.is_some() );

	// 	debug!( "HandleRing: end" );


	// 	// Just a memory barrier.
	// 	//
	// 	// DONE.store( 0, Ordering::SeqCst );
	// }
}


// Each node will start by sending a 1 to the next node. When the counter has come back and
// been incremented by all nodes, the operation is complete.
//
pub struct HandleNode
{
	id: usize                 ,
	n : usize                 ,
	tx: mpsc::Sender  <usize> ,
	rx: mpsc::Receiver<usize> ,
}

impl HandleNode
{
	async fn run( &mut self, exec: impl SpawnHandle<()> )
	{
		debug!( "HandleNode {}: run", self.id );

		let mut tx = self.tx.clone();
		exec.spawn_handle( async move { tx.send( 1 ).await.expect( "Node: send initial message" ); } )

			.expect( "spawn forward" ).await;

		debug!( "HandleNode {}: start loop", self.id );

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
				trace!( "HandleNode {}: received our own message back", self.id );

				// Store the fact that we are done.
				//
				let old = DONE.fetch_add( 1, Ordering::SeqCst );

				// If we are the last one.
				//
				if old+1 == self.n
				{
					trace!( "HandleNode {}: all done", self.id );

					break;
				}

				// else continue passing on messages for the other nodes.
				//
				continue;
			}

			trace!( "HandleNode {}: forwarding a message", self.id );

			// forward the message
			//
			let mut tx = self.tx.clone();

			exec.spawn_handle( async move { tx.send( msg + 1 ).await.expect( "HandleNode: forward message" ); } )

				.expect( "spawn forward" ).await;
		}

		trace!( "HandleNode {}: close our sender", self.id );

		// Allow nodes that where still passing on nodes to detect that we are done.
		//
		self.tx.close().await.expect( "close channel" );

		debug!( "HandleNode {}: run END", self.id );
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
		let ring2 = HandleRing::new( 2 );
		assert_eq!( 2, ring2.nodes.unwrap().len() );

		let ring2 = HandleRing::new( 3 );
		assert_eq!( 3, ring2.nodes.unwrap().len() );
	}
}
