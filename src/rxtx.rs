
use crate::payload::Payload;

pub struct Received {
    pub pipe: u8,
    pub payload: Payload,
}

pub struct SendReceiveResult {
    pub received: Option<Received>,
    pub sent: bool,
    /// When a packet is unacknowledged after it's maximum retries, this flag is set
    pub dropped: bool,
}
