#![allow(unused)]

use std::{
    ffi::CString,
    hash::{DefaultHasher, Hash, Hasher},
    sync::{
        atomic::{AtomicUsize, Ordering::Acquire, Ordering::Release},
        Arc, RwLock,
    },
    usize,
};

use libc::{
    ftruncate, mmap, pthread_mutex_init, pthread_mutex_lock, pthread_mutex_t,
    pthread_mutex_trylock, pthread_mutex_unlock, pthread_mutexattr_init,
    pthread_mutexattr_setpshared, pthread_mutexattr_t, shm_open, MAP_SHARED, O_CREAT, O_RDWR,
    PROT_READ, PROT_WRITE, PTHREAD_PROCESS_SHARED, S_IRUSR, S_IWUSR,
};

pub const THREAD_NUM: usize = 8;

const FIELD_SIZE: usize = std::mem::size_of::<MessageField>();

static SEQ: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Empty,                //placeholder for an empty space in the message queue
    Insert(usize, usize), //inserts a key value pair (key, value)
    Read(usize),          //gets the value assosiated with a key. (key)
    Value(usize),         //positive response to the Read message (value)
    Fail,                 //negative response to the Read message
    Delete(usize),        //deletes all key-value pairs with this key (key)
    Print(usize),         //prints the bucket at the index (index)
    Quit,                 //stops the server
}

pub struct HashTable {
    buckets: Vec<RwLock<Vec<(usize, usize)>>>,
}

impl HashTable {
    //creates a new hashtable with size buckets
    pub fn new(size: usize) -> Self {
        let mut buckets = Vec::new();
        for _ in 0..size {
            let bucket = RwLock::new(Vec::new());
            buckets.push(bucket);
        }
        HashTable { buckets }
    }

    //Inserts a new key - value pair into the HashTable, duplicate keys are allowed
    pub fn insert(&self, key: usize, value: usize) {
        let mut bucket = self.get_bucket(key).write().unwrap();
        SEQ.fetch_add(1, Release);
        bucket.push((key, value));
    }

    //Deletes all occurences of the specified key from the HashTable
    pub fn delete(&self, key: usize) {
        let mut bucket = self.get_bucket(key).write().unwrap();
        SEQ.fetch_add(1, Release);
        bucket.retain(|kv| kv.0 != key);
    }

    //Returns Some(value) if the key is present, else returns None
    pub fn read(&self, key: usize) -> Option<usize> {
        let bucket = self.get_bucket(key).read().unwrap();
        SEQ.fetch_add(1, Release);
        match bucket.iter().find(|kv| kv.0 == key) {
            None => None,
            Some(kv) => Some(kv.1),
        }
    }

    //Prints the contents of the Bucket at the given index to stdout
    pub fn print(&self, index: usize) {
        match self.buckets.get(index) {
            None => println!("Bucket with the index {} does not exist", index),
            Some(lock) => {
                println!("Bucket {}:", index);
                let guard = lock.read().unwrap();
                SEQ.fetch_add(1, Release);
                guard.iter().for_each(|kv| println!("{:?}", kv));
            }
        }
    }

    fn get_bucket(&self, key: usize) -> &RwLock<Vec<(usize, usize)>> {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        let index = hasher.finish() as usize % self.buckets.len();
        self.buckets.get(index).unwrap()
    }
}

pub struct OperationSlot {
    lock: pthread_mutex_t,
    lock_attr: pthread_mutexattr_t,
    op: Operation,
    seq: usize,
    has_work: bool,
    has_result: bool,
}

impl OperationSlot {
    fn perform_work(&mut self, ht: Arc<HashTable>) {
        match self.op {
            Operation::Insert(k, v) => {
                ht.insert(k, v);
                self.op = Operation::Empty;
            }
            Operation::Delete(k) => {
                ht.delete(k);
                self.op = Operation::Empty;
            }
            Operation::Read(k) => match ht.read(k) {
                Some(v) => {
                    self.op = Operation::Value(v);
                    self.has_result = true;
                }
                None => {
                    self.op = Operation::Fail;
                    self.has_result = true;
                }
            },
            Operation::Print(i) => {
                ht.print(i);
                self.op = Operation::Empty;
            }
            Operation::Quit => std::process::exit(0),
            _ => self.op = Operation::Empty,
        }
    }
}

pub struct MessageField {
    slots: [OperationSlot; THREAD_NUM],
}

impl MessageField {
    pub fn get_work(&mut self, ht: Arc<HashTable>, id: usize) {
        let slot = &mut self.slots[id];
        loop {
            unsafe {
                if pthread_mutex_trylock(&mut slot.lock) == 0 {
                    if slot.has_work && !slot.has_result && (SEQ.load(Acquire)) == slot.seq {
                        let ht = ht.clone();
                        slot.perform_work(ht);
                        slot.has_work = false;
                    }
                    pthread_mutex_unlock(&mut slot.lock);
                }
            }
        }
    }

    pub fn put_work(&mut self, op: Operation, seq: usize, id: usize) -> usize {
        loop {
            let slot = &mut self.slots[id];
            unsafe {
                if pthread_mutex_trylock(&mut slot.lock) == 0 {
                    if !slot.has_work && !slot.has_result {
                        slot.op = op;
                        slot.has_work = true;
                        slot.seq = seq;
                        pthread_mutex_unlock(&mut slot.lock);
                        return id;
                    }
                    pthread_mutex_unlock(&mut slot.lock);
                }
            }
        }
    }

    //picks up the
    pub fn pick_up_result(&mut self, id: usize) -> Operation {
        let slot = &mut self.slots[id];
        let op;
        loop {
            unsafe {
                pthread_mutex_lock(&mut slot.lock);
                if slot.has_result == true {
                    op = slot.op;
                    slot.op = Operation::Empty;
                    slot.has_result = false;
                    pthread_mutex_unlock(&mut slot.lock);
                    break;
                }
                pthread_mutex_unlock(&mut slot.lock);
            }
        }
        op
    }
}

//creates and initializes the MessageField struct in POSIX shared memory.
pub fn create_message_field(server: bool) -> *mut MessageField {
    let name = CString::new("msgfield").unwrap();
    let addr;
    unsafe {
        let fd = shm_open(name.as_ptr(), O_CREAT | O_RDWR, S_IRUSR | S_IWUSR);
        ftruncate(fd, FIELD_SIZE as i64);
        addr = mmap(
            std::ptr::null_mut(),
            FIELD_SIZE,
            PROT_READ | PROT_WRITE,
            MAP_SHARED,
            fd,
            0,
        ) as *mut MessageField;
        if server {
            for slot in (*addr).slots.iter_mut() {
                let attr = &mut (*slot).lock_attr;
                let lock = &mut (*slot).lock;
                pthread_mutexattr_init(attr);
                pthread_mutexattr_setpshared(attr, PTHREAD_PROCESS_SHARED);
                pthread_mutex_init(lock, attr);
                slot.has_work = false;
                slot.has_result = false;
                slot.seq = 0;
            }
        }
    }
    addr
}
