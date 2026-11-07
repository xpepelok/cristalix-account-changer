use std::sync::Arc;

use ac_api::{BackendEvent, EventStream};
use futures::StreamExt;
use futures::channel::mpsc::{UnboundedSender, unbounded};
use parking_lot::Mutex;

#[derive(Clone, Default)]
pub(crate) struct Events {
    subscribers: Arc<Mutex<Vec<UnboundedSender<BackendEvent>>>>,
}

impl Events {
    pub fn subscribe(&self) -> EventStream {
        let (sender, receiver) = unbounded();
        self.subscribers.lock().push(sender);
        receiver.boxed()
    }

    pub fn emit(&self, event: BackendEvent) {
        self.subscribers
            .lock()
            .retain(|sender| sender.unbounded_send(event.clone()).is_ok());
    }
}
