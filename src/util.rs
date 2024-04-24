use std::{
    ffi::c_char,
    hash::{DefaultHasher, Hash, Hasher},
    sync::RwLock,
};

use libc::{
    c_void, ftruncate, memset, mmap, pthread_cond_init, pthread_cond_signal, pthread_cond_t,
    pthread_cond_wait, pthread_condattr_init, pthread_condattr_setpshared, pthread_condattr_t,
    pthread_mutex_init, pthread_mutex_lock, pthread_mutex_t, pthread_mutex_unlock,
    pthread_mutexattr_init, pthread_mutexattr_setpshared, pthread_mutexattr_t, shm_open,
    shm_unlink, MAP_SHARED, O_CREAT, O_RDWR, PROT_READ, PROT_WRITE, PTHREAD_PROCESS_SHARED,
    S_IRUSR, S_IWUSR,
};

const SHM_NAME: &str = "ipcmem";
const QUEUE_LEN: usize = 100;
const MEM_SIZE: usize = std::mem::size_of::<MessageQueue>();

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Operation {
    Nop,
    Insert(usize, usize),
    Read(usize),
    Delete(usize),
    Print(usize),
    Quit,
}

pub struct HashTable(Vec<RwLock<Vec<(usize, usize)>>>);

impl HashTable {
    pub fn new(size: usize) -> Self {
        let mut buckets = Vec::new();
        for _ in 0..size {
            let bucket = RwLock::new(Vec::new());
            buckets.push(bucket);
        }
        HashTable(buckets)
    }

    //Inserts a new key - value pair into the HashTable, duplicates are not allowed
    pub fn insert(&self, key: usize, value: usize) {
        let mut bucket = self.get_bucket(key).write().unwrap();
        if bucket.iter().position(|kv| kv.0 == key).is_some() {
            eprintln!("Duplicate key, not inserting");
        } else {
            bucket.push((key, value));
        }
    }

    //Deletes the specified key and its value from the HashTable
    pub fn delete(&self, key: usize) {
        let mut bucket = self.get_bucket(key).write().unwrap();
        match bucket.iter().position(|kv| kv.0 == key) {
            None => return,
            Some(index) => bucket.remove(index),
        };
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
        match self.0.get(index) {
            None => println!("Bucket with the index {} does not exist", index),
            Some(lock) => {
                println!("Bucket {}:", index);
                let guard = lock.read().unwrap();
                guard.iter().for_each(|kv| println!("{:?}", kv));
            }
        }
    }

    fn get_bucket(&self, key: usize) -> &RwLock<Vec<(usize, usize)>> {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        let index = hasher.finish() as usize % self.0.len();
        self.0.get(index).unwrap()
    }
}

pub struct MessageQueue {
    lock: pthread_mutex_t,
    mattr: pthread_mutexattr_t,
    nempty: pthread_cond_t,
    nempty_attr: pthread_condattr_t,
    full: pthread_cond_t,
    full_attr: pthread_condattr_t,
    queue: [Operation; QUEUE_LEN],
    tail: usize, //points to the first element to handle
    free: usize, //points to the first free slot
    len: usize,
}

impl MessageQueue {
    pub unsafe fn enqueue(&mut self, op: Operation) {
        let mutex = &mut (self.lock) as *mut pthread_mutex_t;
        let nempty = &mut (self.nempty) as *mut pthread_cond_t;
        let full = &mut (self.full) as *mut pthread_cond_t;
        pthread_mutex_lock(mutex);
        loop {
            if self.len < QUEUE_LEN {
                self.queue[self.free] = op;
                self.free = (self.free + 1) % QUEUE_LEN;
                self.len += 1;
                break;
            } else {
                pthread_cond_wait(full, mutex);
            }
        }
        pthread_mutex_unlock(mutex);
        pthread_cond_signal(nempty);
    }

    pub unsafe fn dequeue(&mut self) -> Operation {
        let mutex = &mut (self.lock) as *mut pthread_mutex_t;
        let nempty = &mut (self.nempty) as *mut pthread_cond_t;
        let full = &mut (self.full) as *mut pthread_cond_t;
        pthread_mutex_lock(mutex);
        let msg = loop {
            if self.queue[self.tail] != Operation::Nop {
                break self.queue[self.tail];
            } else {
                pthread_cond_wait(nempty, mutex);
            }
        };
        self.queue[self.tail] = Operation::Nop;
        self.tail = (self.tail + 1) % QUEUE_LEN;
        self.len -= 1;
        pthread_mutex_unlock(mutex);
        pthread_cond_signal(full);
        msg
    }
}

pub(crate) unsafe fn create_msq(server: bool) -> *mut MessageQueue {
    let fd = shm_open(
        String::from(SHM_NAME).as_ptr() as *mut c_char,
        O_CREAT | O_RDWR,
        S_IRUSR | S_IWUSR,
    );
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
        let nempty_attr = &mut (*addr).nempty_attr as *mut pthread_condattr_t;
        let nempty = &mut (*addr).nempty as *mut pthread_cond_t;
        let full = &mut (*addr).full as *mut pthread_cond_t;
        let full_attr = &mut (*addr).full_attr as *mut pthread_condattr_t;
        pthread_mutexattr_init(mattr);
        pthread_mutexattr_setpshared(mattr, PTHREAD_PROCESS_SHARED);
        pthread_mutex_init(mutex, mattr);
        pthread_condattr_init(nempty_attr);
        pthread_condattr_setpshared(nempty_attr, PTHREAD_PROCESS_SHARED);
        pthread_cond_init(nempty, nempty_attr);
        pthread_condattr_init(full_attr);
        pthread_condattr_setpshared(full_attr, PTHREAD_PROCESS_SHARED);
        pthread_cond_init(full, full_attr);
        (*addr).free = 0;
        (*addr).tail = 0;
        (*addr).len = 0;
    }
    addr
}

pub unsafe fn destroy_shm() {
    shm_unlink(String::from(SHM_NAME).as_ptr() as *mut c_char);
}
