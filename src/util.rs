use std::{
    collections::VecDeque,
    ffi::CString,
    hash::{DefaultHasher, Hash, Hasher},
    sync::RwLock,
    usize,
};

use libc::{
    c_void, ftruncate, memset, mmap, pthread_cond_init, pthread_cond_signal, pthread_cond_t,
    pthread_cond_wait, pthread_condattr_init, pthread_condattr_setpshared, pthread_condattr_t,
    pthread_mutex_init, pthread_mutex_lock, pthread_mutex_t, pthread_mutex_unlock,
    pthread_mutexattr_init, pthread_mutexattr_setpshared, pthread_mutexattr_t, shm_open,
    MAP_SHARED, O_CREAT, O_RDWR, PROT_READ, PROT_WRITE, PTHREAD_PROCESS_SHARED, S_IRUSR, S_IWUSR,
};

const QUEUE_LEN: usize = 4096;
const MEM_SIZE: usize = std::mem::size_of::<MessageQueue>();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Message {
    Empty,                //placeholder for an empty space in the message queue
    Insert(usize, usize), //inserts a key value pair
    Read(usize),          //gets the value assosiated with a key.
    Value(usize),         //positive response to the Read message
    Fail,                 //negative response to the Read message
    Delete(usize),        //deletes all key-value pairs with this key
    Print(usize),         //prints the bucket at the index
    ThStart(usize),       //starts n worker threads
    ThStop,               //stops the thread that receives the message
    Quit,                 //stops the server
}

pub struct HashTable {
    buckets: Vec<RwLock<VecDeque<(usize, usize)>>>,
}

impl HashTable {
    pub fn new(size: usize) -> Self {
        let mut buckets = Vec::new();
        for _ in 0..size {
            let bucket = RwLock::new(VecDeque::new());
            buckets.push(bucket);
        }
        HashTable { buckets }
    }

    //Inserts a new key - value pair into the HashTable, duplicate keys are allowed
    pub fn insert(&self, key: usize, value: usize) {
        let mut bucket = self.get_bucket(key).write().unwrap();
        bucket.push_front((key, value));
    }

    //Deletes all occurences of the specified key from the HashTable
    pub fn delete(&self, key: usize) {
        let mut bucket = self.get_bucket(key).write().unwrap();
        bucket.retain(|kv| kv.0 != key);
    }

    //Returns Some(value) if the key is present, else returns None
    pub fn read(&self, key: usize) -> Option<usize> {
        let bucket = self.get_bucket(key).read().unwrap();
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
                guard.iter().for_each(|kv| println!("{:?}", kv));
            }
        }
    }

    fn get_bucket(&self, key: usize) -> &RwLock<VecDeque<(usize, usize)>> {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        let index = hasher.finish() as usize % self.buckets.len();
        self.buckets.get(index).unwrap()
    }
}

//A message queue for communication through a segment of shared memory, works like a ring buffer.
pub struct MessageQueue {
    lock: pthread_mutex_t,
    mattr: pthread_mutexattr_t,
    empty: pthread_cond_t, //signals that the queue is not empty
    empty_attr: pthread_condattr_t,
    full: pthread_cond_t, //signals that the queue is not full
    full_attr: pthread_condattr_t,
    queue: [Message; QUEUE_LEN],
    tail: usize, //points to the first element to handle
    free: usize, //points to the first free slot
}

impl MessageQueue {
    //enqueues a message in the message queue, if the queue is full, wait until it is not.
    pub unsafe fn enqueue(&mut self, op: Message) {
        let mutex = &mut (self.lock) as *mut pthread_mutex_t;
        let empty = &mut (self.empty) as *mut pthread_cond_t;
        let full = &mut (self.full) as *mut pthread_cond_t;
        pthread_mutex_lock(mutex);
        loop {
            if self.queue[self.free] == Message::Empty {
                self.queue[self.free] = op;
                self.free = (self.free + 1) % QUEUE_LEN;
                break;
            } else {
                pthread_cond_wait(full, mutex);
            }
        }
        pthread_mutex_unlock(mutex);
        pthread_cond_signal(empty);
    }

    //dequeues a message from the message queue, if the queue is empty, wait until it is not
    pub unsafe fn dequeue(&mut self) -> Message {
        let mutex = &mut (self.lock) as *mut pthread_mutex_t;
        let empty = &mut (self.empty) as *mut pthread_cond_t;
        let full = &mut (self.full) as *mut pthread_cond_t;
        pthread_mutex_lock(mutex);
        let msg = loop {
            if self.queue[self.tail] != Message::Empty {
                break self.queue[self.tail];
            } else {
                pthread_cond_wait(empty, mutex);
            }
        };
        self.queue[self.tail] = Message::Empty;
        self.tail = (self.tail + 1) % QUEUE_LEN;
        pthread_mutex_unlock(mutex);
        pthread_cond_signal(full);
        msg
    }
}

//sets up a segment of POSIX shared memory and initializes it as a MessageQueue
pub unsafe fn create_msq(server: bool, to_server: bool) -> *mut MessageQueue {
    let name = CString::new(if to_server { "to_server" } else { "to_client" }).unwrap();
    let fd = if !to_server {
        shm_open(name.as_ptr(), O_CREAT | O_RDWR, S_IRUSR | S_IWUSR)
    } else {
        shm_open(name.as_ptr(), O_CREAT | O_RDWR, S_IRUSR | S_IWUSR)
    };
    ftruncate(fd, MEM_SIZE as i64);
    let addr = mmap(
        std::ptr::null_mut(),
        MEM_SIZE,
        PROT_READ | PROT_WRITE,
        MAP_SHARED,
        fd,
        0,
    ) as *mut MessageQueue;
    if server {
        memset(addr as *mut c_void, 0, MEM_SIZE);
        let mattr = &mut (*addr).mattr as *mut pthread_mutexattr_t;
        let mutex = &mut (*addr).lock as *mut pthread_mutex_t;
        let empty_attr = &mut (*addr).empty_attr as *mut pthread_condattr_t;
        let emtpy = &mut (*addr).empty as *mut pthread_cond_t;
        let full = &mut (*addr).full as *mut pthread_cond_t;
        let full_attr = &mut (*addr).full_attr as *mut pthread_condattr_t;
        pthread_mutexattr_init(mattr);
        pthread_mutexattr_setpshared(mattr, PTHREAD_PROCESS_SHARED);
        pthread_mutex_init(mutex, mattr);
        pthread_condattr_init(empty_attr);
        pthread_condattr_setpshared(empty_attr, PTHREAD_PROCESS_SHARED);
        pthread_cond_init(emtpy, empty_attr);
        pthread_condattr_init(full_attr);
        pthread_condattr_setpshared(full_attr, PTHREAD_PROCESS_SHARED);
        pthread_cond_init(full, full_attr);
        (*addr).free = 0;
        (*addr).tail = 0;
    }
    addr
}
