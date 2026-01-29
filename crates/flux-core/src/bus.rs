use tokio::sync::broadcast;
use flux_types::message::Message;
use std::sync::Arc;

#[derive(Clone)]
pub struct EventBus {
    sender: broadcast::Sender<Message>,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Message> {
        self.sender.subscribe()
    }

    pub fn publish(&self, message: Message) -> Result<usize, broadcast::error::SendError<Message>> {
        self.sender.send(message)
    }
}

pub type SharedEventBus = Arc<EventBus>;
