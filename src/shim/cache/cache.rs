use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use futures::channel::mpsc;
use std::sync::Mutex;
use futures_core::{Stream, Poll};
use bytes::Bytes;

struct Cache {
    heapcache Mutxe<BTreeMap<String, Vec<u8>>>,
    ready: AtomicBool,
    size: 
    waiters: Mutex<&Vec<Sender>>,
}

impl Cache {

    // TODO - get content-length and pass in to create vec of the right size
    pub async fn store(&self, key: String, body: Stream<Item = Result<Bytes>>, len: i32) Result {
        if !self.ready.load(Ordering::SeqCst) {
            self.wait_ready().await;
        }

        let data = Vec::with_capacity(len as usize)
        while let Poll::Ready(Some(b)) = body.poll_next().await? {
            data.append(b.clone());   
        }
        
        self.heapcache.lock().unwrap()
        self.heapcache.insert(key, data);
    }

    // TODO stub for now
    async fn wait_ready(&self) {
        let (send, receive) = mpsc::unbounded();
        self.waiters.lock().unwrap();
        self.waiters.push(send);
        receive.next().await;
    }

    // implements an LRU algoritihm to clear out the cache as needed
    fn lru(&self) {

    }

}