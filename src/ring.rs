use futures::channel::mpsc::{ unbounded, UnboundedReceiver, UnboundedSender };
use futures::{ SinkExt, StreamExt, task::{ Spawn, SpawnExt } };
use std::{ sync::{ atomic::{ AtomicUsize, Ordering } } };
use log::*;

static DONE: AtomicUsize = AtomicUsize::new( 0 );


pub struct Ring
{
	nodes: Option< Vec<Node> > ,
}


impl Ring
{
	// Create channels between all the nodes.
	//
	pub fn new( n: usize ) -> Self
	{
		assert!( n > 1 );

		let mut nodes = Vec::with_capacity( n );

		// The connection between the last and the first node
		//
		let (last_tx, last_rx) = unbounded();

		// The first node
		//
		let (tx, mut next_rx) = unbounded();

		nodes.push( Node { id: 1, n, tx, rx: last_rx } );


		// All but first and last.
		// Note that 1..1 does not do anything, so it works if n==2.
		//
		for id in 2..n
		{
			let (tx, rx) = unbounded();

			nodes.push( Node { id, n, tx, rx: next_rx } );

			next_rx = rx;
		}


		// The last node
		//
		nodes.push( Node { id: n, n, tx: last_tx, rx: next_rx } );

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
	pub async fn run( &mut self, exec: &mut impl Spawn )
	{
		debug!( "Ring: start" );

		// Need to reset the DONE counter, since it might be reused on repeated benchmarks.
		//
		DONE.store( 0, Ordering::SeqCst );


		let (done_tx, mut done_rx) = unbounded();


		for mut node in self.nodes.take().unwrap().into_iter()
		{
			let done_tx = done_tx.clone();

			exec.spawn( async move { node.run( done_tx ).await; } ).expect( "spawn node" );
		};

		let res = done_rx.next().await;

		debug_assert!( res.is_some() );

		debug!( "Ring: end" );


		// Just a memory barrier.
		//
		// DONE.store( 0, Ordering::SeqCst );
	}
}


// Each node will start by sending a 1 to the next node. When the counter has come back and
// been incremented by all nodes, the operation is complete.
//
pub struct Node
{
	id: usize                    ,
	n : usize                    ,
	tx: UnboundedSender  <usize> ,
	rx: UnboundedReceiver<usize> ,
}

impl Node
{
	async fn run( &mut self, mut done_tx: UnboundedSender<()> )
	{
		debug!( "Node {}: run", self.id );

		self.tx.unbounded_send( 1 ).expect( "Node: send initial message" );


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
				trace!( "Node {}: received our own message back", self.id );

				// Store the fact that we are done.
				//
				let old = DONE.fetch_add( 1, Ordering::SeqCst );

				// If we are the last one.
				//
				if old+1 == self.n
				{
					trace!( "Node {}: all done", self.id );

					done_tx.send(()).await.expect( "Send DONE" );

					break;
				}

				// else continue passing on messages for the other nodes.
				//
				continue;
			}

			trace!( "Node {}: forwarding a message", self.id );

			// forward the message
			//
			self.tx.unbounded_send( msg + 1 ).expect( "Node: forward message" );
		}

		trace!( "Node {}: close our sender", self.id );

		// Allow nodes that where still passing on nodes to detect that we are done.
		//
		self.tx.close().await.expect( "close channel" );

		debug!( "Node {}: run END", self.id );
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
		let ring2 = Ring::new( 2 );
		assert_eq!( 2, ring2.nodes.unwrap().len() );

		let ring2 = Ring::new( 3 );
		assert_eq!( 3, ring2.nodes.unwrap().len() );
	}
}
