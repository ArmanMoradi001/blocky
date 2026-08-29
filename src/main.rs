use std::time::SystemTime;

pub struct Block {
    nonce: i32,
    previous_hash: Vec<u8>,
    time_stamp: u128,
    transactions: Vec<Vec<u8>>,
}

impl Block {
    pub fn new(nonce: i32, previous_hash: Vec<u8>) -> Self {


        let time_now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap();

        Block { nonce, previous_hash, time_stamp: time_now.as_nanos(), transactions: Vec::<Vec<u8>>::new() }
    }

    pub fn print_block(&self){
        println!("This is previous hash: {:?}", self.previous_hash);
        println!("This is nonce: {}", self.nonce);
        println!("This is transactions vector: {:?}", self.transactions);
        println!("This is time stamp: {:x}", self.time_stamp)
    }
}

fn main() {

    let b1 = Block::new(0, "This is our first block in this blockchain".to_string().into_bytes());
    b1.print_block();
}
