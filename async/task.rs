#![allow(unused)]

use std::collections::HashSet;
//
// # Exercise Expectations
//
// The goal here to is to establish a basis for our technical interview,
// and be able to discuss the choices you made and why. We're far more interested
// in the discussion part than receiving a perfect implementation of the following
// exercise. The exercise is not designed to take too long of your personal time,
// and doesn't have to be completed fully (although, bonus point if it is). We estimate
// it should be achieveable to complete between 2 to 3 hours of dedicated time.
//
// Please keep this exercise private, and don't make your result available publically.
//
// ## Recap about Blockchain
//
// A blockchain is organised as sequence of blocks.
//
//      Block 0 (Genesis) <- Block 1 <- Block 2 <- ...
//
// Block i is a parent of Block i+1
// Block i+1 is a child of Block i
//
// Each block has a specific hash, that are considered unique, and each blocks contains a reference to
// its parent block's hash.
//
// The first block is called genesis, and doesn't have a parent; this is the oldest block in the chain.
// The latest block is often called the tip of the chain and is the yougest block added to the chain.
//
// # Block streaming protocol
//
// Design a simple wire protocol to stream a sequence of blocks.
// For the purposes of this exercise, each block is represented with a
// hash of the parent block (all zeros can be used in the genesis block),
// a block number, and an opaque data blob for content.


use std::pin::Pin;
use std::marker::Unpin;
use std::task::Context;
use std::task::Poll;

use futures::executor::block_on;
use futures::io;
use futures::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    // Block number, monotonically increasing as the chain grows.
    block_number: u64,
    // Hash of the parent block.
    parent_hash: [u8; 32],
    // Block content.
    content: Box<[u8]>,
}

// #[pin_project::pin_project]
pub struct BlockStream<R: AsyncRead> {
    // #[pin]
    io: Pin<Box<R>>,
}


//******************************************************************************
// Part 1.1: define a function to read a stream of blocks from a generic
// asynchronous input source.
//******************************************************************************


pub fn read_blocks<R: AsyncRead>(io: R) -> BlockStream<R> {
    BlockStream { io: Box::pin(io) }
}


impl<R: AsyncRead> Stream for BlockStream<R> {
    type Item = Result<Block, io::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut buffer = [0u8; 56];
        match Pin::new(&mut self.io).poll_read(cx, &mut buffer) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Ok(0)) => Poll::Ready(None),
            Poll::Ready(Ok(n)) if n == buffer.len() => {
                let block = Block {
                    block_number: u64::from_le_bytes(buffer[0..8].try_into().unwrap()),
                    parent_hash: buffer[8..40].try_into().unwrap(),
                    content: buffer[40..].to_vec().into_boxed_slice(),
                };
                Poll::Ready(Some(Ok(block)))
            }
            Poll::Ready(Ok(c)) => {
                Poll::Ready(Some(Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Incomplete block data",
                ))))
            }
            Poll::Ready(Err(e)) => Poll::Ready(Some(Err(e))),
        }
    }
}

// The returned object should be usable in an async loop like this:
//
//  async {
//      let mut stream = read_blocks(io);
//      while let Some(res) = stream.next().await {
//          let block: Block = res?;
//          // ...
//      }
//  }
//
// It is allowed to use available libraries from e.g. crates.io for the
// implementation, but BlockStream should not be an alias to a foreign type.


/// Part 1.2: Given a list of streamed block chains, return the block that is
/// the common ancestor if it exists, or None if there is no common ancestor
/// to all of the chains.
///
/// The streams are assumed to yield blocks of valid chains in
/// descendant-to-ancestor order, i.e. in each stream the block numbers decrease
/// monotonically and a parent hash is identical to one computed
/// from the blocks in the rest of the stream.
pub async fn find_common_ancestor<R: AsyncRead>(blockchain_streams: &mut [BlockStream<R>]) -> Result<Option<Block>, io::Error> {
    let mut recent_blocks: Vec<Option<Block>> = Vec::new();

    // Iterate through each stream and initialize the latest block.
    for stream in blockchain_streams.iter_mut() {
        match stream.next().await {
            Some(Ok(block)) => recent_blocks.push(Some(block)),
            Some(Err(e)) => return Err(e),
            None => return Ok(None),
        }
    }

    // Keep looping until there is only one block left or until common ancestor is found.
    loop {
        // Get the min_block where this is the common ancestor.
        let min_block = recent_blocks
            .iter()
            .map(|block| block.as_ref().unwrap().block_number)
            .min();

        match min_block {
            Some(min) => {
                
                let vec_blocks_min: Vec<&Block> = recent_blocks
                    .iter()
                    .filter_map(|block| block.as_ref().filter(|b| b.block_number == min))
                    .collect();

                // If all the block with the minimum block number found in stream, it's the common ancestor.
                if vec_blocks_min.len() == 1 {
                    if let Some(ans) = vec_blocks_min.first().map(|b| (*b).clone()) {
                        return Ok(Some(ans));
                    } 
                }

                // If there are multiple blocks with the minimum block number, check parent hashes.
                let parent_hashes: Vec<[u8; 32]> = vec_blocks_min
                    .iter()
                    .map(|block| block.parent_hash)
                    .collect();

                // If all parent hashes are the same, it's the common ancestor.
                if parent_hashes.iter().all(|&hash| hash == parent_hashes[0]) {
                    if let Some(ans) = vec_blocks_min.first().map(|b| (*b).clone()) {
                        return Ok(Some(ans));
                    } 
                }

                // Update the latest blocks with the next block from each stream.
                for (i, stream) in blockchain_streams.iter_mut().enumerate() {
                    if let Some(Ok(block)) = stream.next().await {
                        recent_blocks[i] = Some(block);
                    } else {
                        return Ok(None);
                    }
                }
            }
            None => return Ok(None),
        }
    }
}

//********************************************************************************
// Part 2: Tests
//
// * Describe your process to tests some of the functions and properties.
// * Either in the form of valid rust code, or commented pseudo code.
//********************************************************************************
#[cfg(test)]
mod tests {
    use super::*;
    use async_std::io::Cursor;

    fn create_random_block() -> Block {
        let mut buffer = [0u8; 56];
        let block = Block {
            block_number: u64::from_le_bytes(buffer[0..8].try_into().unwrap()),
            parent_hash: buffer[8..40].try_into().unwrap(),
            content: buffer[40..].to_vec().into_boxed_slice(),
        };
        block
    }
    
    #[tokio::test]
    async fn test_read_blocks() {
        // let block = create_random_block();
        let data = vec![0;56];
        let cursor = Cursor::new(data);

        let mut stream = read_blocks(cursor);
        while let Some(res) = stream.next().await {
            let block: Block = res.unwrap();
            assert_eq!(block.block_number, 0);
        }
    }

    #[tokio::test]
    async fn test_find_common_ancestor() {
        let mut chain1: Vec<u8> = vec![1, 2, 3, 4, 5, 6, 7, 8, 9];
        chain1.reverse();
        let mut chain2: Vec<u8> = vec![1, 2, 3, 9, 10, 11, 13, 14, 15, 16];
        chain2.reverse();
        let mut chain3: Vec<u8> = vec![1, 2, 3, 17, 18, 19, 20, 21];
        chain3.reverse();

        let mut streams: Vec<BlockStream<Cursor<Vec<u8>>>> = vec![
            BlockStream { io: Box::pin(Cursor::new(chain1)) },
            BlockStream { io: Box::pin(Cursor::new(chain2)) },
            BlockStream { io: Box::pin(Cursor::new(chain3)) },
        ];

        let common_ancestor = find_common_ancestor(&mut streams).await.unwrap();
        assert_eq!(common_ancestor, Some(Block {
            block_number: 3,
            parent_hash: [0u8; 32],
            content: vec![1, 2, 3].into_boxed_slice(),
        }));
    }

}

fn main() {

}
