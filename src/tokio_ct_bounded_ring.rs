//! This uses bounded channels and has the possibility to spawn each message forwarding on the executor
//! rather than awaiting.
//!
use futures::{ future::join_all, SinkExt, StreamExt, channel::mpsc };
use std::{ sync::{ atomic::{ AtomicUsize, Ordering } } };
use log::*;
use tokio::task;

static DONE: AtomicUsize = AtomicUsize::new( 0 );


pub struct TokioCtNativeRing
{
	nodes: Option< Vec<TokioCtNativeNode> > ,
}


impl TokioCtNativeRing
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

		nodes.push( TokioCtNativeNode { id: 1, n, tx, rx: last_rx } );


		// All but first and last.
		// Note that 1..1 does not do anything, so it works if n==2.
		//
		for id in 2..n
		{
			let (tx, rx) = mpsc::channel( channel_size );

			nodes.push( TokioCtNativeNode { id, n, tx, rx: next_rx } );

			next_rx = rx;
		}


		// The last node
		//
		nodes.push( TokioCtNativeNode { id: n, n, tx: last_tx, rx: next_rx } );

		Self
		{
			nodes: Some( nodes ),
		}
	}


	// Run the benchmark.
	//
	pub async fn run( &mut self )
	{
		debug!( "TokioCtNativeRing: start" );

		// Need to reset the DONE counter, since it might be reused on repeated benchmarks.
		//
		DONE.store( 0, Ordering::SeqCst );

		let mut handles = Vec::with_capacity( self.nodes.as_ref().unwrap().len() );

		for mut node in self.nodes.take().unwrap().into_iter()
		{
			let node = async move { node.run().await; };
			handles.push( task::spawn_local( node ) );
		};

		join_all( handles ).await;

		debug!( "TokioCtNativeRing: end" );
	}
}


// Each node will start by sending a 1 to the next node. When the counter has come back and
// been incremented by all nodes, the operation is complete.
//
pub struct TokioCtNativeNode
{
	id: usize                 ,
	n : usize                 ,
	tx: mpsc::Sender  <usize> ,
	rx: mpsc::Receiver<usize> ,
}

impl TokioCtNativeNode
{
	async fn run( &mut self )
	{
		debug!( "TokioCtNativeNode {}: run", self.id );

		let mut tx = self.tx.clone();
		task::spawn_local( async move { tx.send( 1 ).await.expect( "Node: send initial message" ); } );

		debug!( "TokioCtNativeNode {}: start loop", self.id );

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
				trace!( "TokioCtNativeNode {}: received our own message back", self.id );

				// Store the fact that we are done.
				//
				let old = DONE.fetch_add( 1, Ordering::SeqCst );

				// If we are the last one.
				//
				if old+1 == self.n
				{
					trace!( "TokioCtNativeNode {}: all done", self.id );

					break;
				}

				// else continue passing on messages for the other nodes.
				//
				continue;
			}

			trace!( "TokioCtNativeNode {}: forwarding a message", self.id );

			// forward the message
			//
			let mut tx = self.tx.clone();

			task::spawn_local( async move { tx.send( msg + 1 ).await.expect( "TokioCtNativeNode: forward message" ); } );
		}

		trace!( "TokioCtNativeNode {}: close our sender", self.id );

		// Allow nodes that where still passing on nodes to detect that we are done.
		//
		self.tx.close().await.expect( "close channel" );

		debug!( "TokioCtNativeNode {}: run END", self.id );
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
		let ring2 = TokioCtNativeRing::new( 2 );
		assert_eq!( 2, ring2.nodes.unwrap().len() );

		let ring2 = TokioCtNativeRing::new( 3 );
		assert_eq!( 3, ring2.nodes.unwrap().len() );
	}
}
