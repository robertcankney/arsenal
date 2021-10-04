use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicBool, Ordering, AtomicU32, AtomicU64};
use futures::channel::mpsc;
use futures_util::stream::StreamExt;
use std::sync::Mutex;
use futures_core::{Stream};
use core::task::Poll;
use bytes::Bytes;
use std::time;
use ulid;
use std::io;
use std::error::Error;

// TODO: implement trait for Cache
pub struct Memstore {
    cache: Mutex<HashMap<String, ValTime>>,
    key_atime: Mutex<BTreeMap<ulid::Ulid, ValSize>>,
    ready: AtomicBool,
    size: AtomicU64,
    soft_limit: i64,
    hard_limit: i64,
    waiters: Mutex<Vec<mpsc::Sender<bool>>>,
}

struct ValTime {
    val: Vec<u8>,
    atime: ulid::Ulid,
}

struct ValSize {
    key: String,
    size: i32,
}

impl Memstore {

    pub async fn store(&self, key: String, body: &Stream<Item=Result<Bytes, Error>>, resp_body: &mut Vec<u8>, len: i32) -> Result<(), Error> {
        if !self.ready.load(Ordering::SeqCst) {
            self.wait_ready().await;
        }
        let mut len = len;

        let mut data = Vec::with_capacity(len as usize);
        let calc = len == 0;
        while let futures_core::Poll::Ready(Some(b)) = body.poll_next().await? {
            data.append(b.clone());
            resp_body.append(b);
            if calc {
                len += b.length();
            }
        }

        let now = time::SystemTime::now().duration_since(time::UNIX_EPOCH)?.as_nanos();
        let ul = ulid::Ulid::from(value: now as u128);
        self.cache.lock().unwrap().insert(key, ValTime{val: data, atime: ulid.clone()});
        self.key_atime.lock().unwrap().insert(ulid, ValSize{key: ul.to_string(), size: len});
        self.size.fetch_add(len as u64, Ordering::SeqCst);
    }

    // retrieve will update key_atime and cache atime on any access, to ensure LRU algorithim remains accurate
    pub fn retrieve(&self, key: &String) -> Option<&Vec<u8>> {
        let val = self.cache.lock().unwrap().get_mut(key.as_str());
        match val {
            None => None,
            Some(v) => {
                let now = time::SystemTime::now().duration_since(time::UNIX_EPOCH)?.as_nanos();
                let ulid = ulid::Ulid::from(now as u128);
                let mut atime = self.key_atime.lock().unwrap();

                atime.remove(key: v.atime);
                v.atime = ulid.clone();
                atime.insert(ulid, ValSize{key: key.clone(), size: v.size});

                Some(&v.val)
            }
        }
    }


    async fn wait_ready(&self) {
        let (send, mut receive) = mpsc::unbounded();
        self.waiters.lock().unwrap();
        self.waiters.push(send);
        receive.next().await?;

    }

    // implements an LRU algorithm to clear out the cache as needed
    async fn lru(&self) -> Result<(), Error> {
        // if we haven't hit soft cache limit, return
        // if we have, but are less than hard, continue
        // if we have passed hard, stop additional stores until we are below soft limit, then continue
        let mut size = match self.size.load(Ordering::SeqCst) {
            Some(n) if n < self.soft_limit => return Ok(()),
            Some(n) if n >= self.soft_limit && n < self.hard_limit => n,
            Some(n) if n >= self.hard_limit => {
                self.ready.store(false, Ordering::SeqCst);
                n
            }
            _ => {} // above is exhaustive but compiler disagrees
        }.ok_or("invalid_size")?;

        let mut klock = self.key_atime.lock().unwrap();
        let mut clock = self.cache.lock().unwrap();
        for (k,v) in klock {
            klock.remove(k);
            clock.remove(v.key);
            size -= v.size;
            if size <= self.soft_limit {
                break;
            }
        }
        self.size.store(size, Ordering::SeqCst);

        let lock = self.waiters.lock().unwrap();
        for w in lock {
            w.try_send(true).await?;
        }

        Ok(self.ready.store(true, Ordering::SeqCst))
    }

}