

use tokio_stream::StreamExt;

#[derive(Debug)]
struct Block(i8);

#[tokio::main]
async fn main() {
    let mut block_stream = tokio_stream::iter(&[Block(1), Block(2), Block(3)]);
    let mut stream = tokio_stream::iter(&[1, 2, 3]);

    while let Some(v) = stream.next().await {
        println!("GOT = {:?}", v);
    }

    while let Some(block) = block_stream.next().await {
        println!("GOT = {:?}", block);
    }
}