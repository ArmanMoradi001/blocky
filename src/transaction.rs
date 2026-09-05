use crate::blockchain::Serialization;
pub struct Transaction{

    sender_address: Vec<u8>,
    reciever_address: Vec<u8>,
    value: u64
}


impl Transaction{

    pub fn new(sender: Vec<u8>, reciever: Vec<u8>, value: u64) -> Self{
        Transaction{
            sender_address: sender,
            reciever_address: reciever,
            value
        }
    }
}


impl Serialization<Transaction> for Transaction{

    fn serialization(&self) -> Vec<u8> {
    let mut bin = Vec::new();

    let sender_len = self.sender_address.len() as u64;
    bin.extend(sender_len.to_be_bytes());
    bin.extend(&self.sender_address);

    let receiver_len = self.reciever_address.len() as u64;
    bin.extend(receiver_len.to_be_bytes());
    bin.extend(&self.reciever_address);

    bin.extend(self.value.to_be_bytes());

    bin
    }


    fn deserialization(bytes: Vec<u8>)-> Transaction {
        let mut pos = 0;

        let need = |pos: usize, n: usize, what: &str| {
            assert!(
                bytes.len() >= pos + n,
                "Transaction::deserialization: truncated {what} (need {} bytes at pos {pos}, got len {})",
                n,
                bytes.len()
            );
        };

        // sender_len: u64 BE
        need(pos, 8, "sender_len");
        let sender_len = u64::from_be_bytes(
            bytes[pos..pos + 8].try_into().unwrap(),
        ) as usize;
        pos += 8;

        // sender_address
        need(pos, sender_len, "sender_address");
        let sender_address = bytes[pos..pos + sender_len].to_vec();
        pos += sender_len;

        // receiver_len: u64 BE
        need(pos, 8, "receiver_len");
        let receiver_len = u64::from_be_bytes(
            bytes[pos..pos + 8].try_into().unwrap(),
        ) as usize;
        pos += 8;

        // receiver_address
        need(pos, receiver_len, "receiver_address");
        let reciever_address = bytes[pos..pos + receiver_len].to_vec();
        pos += receiver_len;

        // value: u64 BE
        need(pos, 8, "value");
        let value = u64::from_be_bytes(bytes[pos..pos + 8].try_into().unwrap());
        pos += 8;

        assert!(
            pos == bytes.len(),
            "Transaction::deserialization: trailing {} byte(s) (len {}, consumed {pos})",
            bytes.len() - pos,
            bytes.len()
        );

        Transaction {
            sender_address,
            reciever_address,
            value,
        }
    }


}