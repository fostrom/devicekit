use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc::{Receiver, Sender, channel},
    },
    thread::{JoinHandle, spawn},
};

type Notification = (String, String);

#[derive(Clone, Debug)]
pub struct NotifyCast {
    next_token: Arc<AtomicU64>,
    listeners: Arc<Mutex<HashMap<u64, Sender<Notification>>>>,
}

impl NotifyCast {
    pub fn new() -> Self {
        Self {
            next_token: Arc::new(AtomicU64::new(0)),
            listeners: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn start_listener(&self, notify_chan_rx: Receiver<Notification>) -> JoinHandle<()> {
        let listeners = self.listeners.clone();

        spawn(move || {
            for (event, data) in notify_chan_rx {
                listeners.lock().unwrap().retain(|_token, listener| {
                    listener.send((event.clone(), data.clone())).is_ok()
                });
            }
        })
    }

    pub fn subscribe(&self) -> (u64, Receiver<Notification>) {
        let (tx, rx) = channel();
        let token = self.incr_token();
        self.listeners.lock().unwrap().insert(token, tx);
        (token, rx)
    }

    pub fn unsubscribe(&self, token: u64) {
        let mut listeners = self.listeners.lock().unwrap();
        listeners.remove(&token);
    }

    fn incr_token(&self) -> u64 {
        self.next_token.fetch_add(1, Ordering::Relaxed)
    }
}
