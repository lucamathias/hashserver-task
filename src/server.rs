mod util;

use std::{
    cmp::min, collections::VecDeque, env::consts::ARCH, process::exit, sync::{Arc, Condvar, Mutex}, thread::spawn
};

use util::{create_msq, HashTable, Message, MessageQueue};

const DEFAULT_SIZE: usize = 1000;
static mut THREAD_NUM: usize = 1;

static mut MSG_IN_BUF: Mutex<VecDeque<Message>> = Mutex::new(VecDeque::new()); // buffers incoming messages
static mut HAS_IN_MSG: Condvar = Condvar::new(); //signals message in MSG_BUF

static mut MSG_OUT_BUF: Mutex<VecDeque<Message>> = Mutex::new(VecDeque::new()); //buffers outgoing messages
static mut HAS_OUT_MSG: Condvar = Condvar::new();

fn main() {
    let size = get_size();
    let hashtable = Arc::new(HashTable::new(size));
    println!(
        "HashTable of size {} created! Waiting for requests...",
        size
    );
    unsafe {
        let outgoing = create_msq(true, false);
        let incoming = create_msq(true, true);
        let tmp_out = outgoing as usize;
        let tmp_in = incoming as usize;
        spawn(move || in_message_thread(tmp_in));
        spawn(move || out_message_thread(tmp_out));
        loop {
            let mut threads = Vec::new();
            for _ in 0..min(THREAD_NUM, 16) {
                let table = hashtable.clone();
                threads.push(spawn(move || worker_thread(table)));
            }
            for thread in threads {
                thread.join().unwrap();
            }
        }
    }
}

fn get_size() -> usize {
    let mut input = String::new();
    println!("Enter the size of the Hashmap:");
    std::io::stdin()
        .read_line(&mut input)
        .expect("failed to read line");
    match input.trim().parse::<usize>() {
        Ok(size) => size,
        Err(_) => {
            eprintln!("Invalid input, defaulting to {}", DEFAULT_SIZE);
            DEFAULT_SIZE
        }
    }
}

unsafe fn in_message_thread(in_ptr: usize) {
    let in_ptr = in_ptr as *mut MessageQueue;
    loop {
        let msg = (*in_ptr).dequeue();
        let mut guard = MSG_IN_BUF.lock().unwrap();
        guard.push_front(msg);
        HAS_IN_MSG.notify_all();
        drop(guard);
    }
}

unsafe fn out_message_thread(out_ptr: usize) {
    let out_ptr = out_ptr as *mut MessageQueue;
    loop {
        let mut guard = MSG_OUT_BUF.lock().unwrap();
        match guard.pop_back() {
            None => {
                let _u = HAS_OUT_MSG.wait(guard).unwrap();
            }
            Some(msg) => (*out_ptr).enqueue(msg),
        }
    }
}

unsafe fn pop_msg() -> Message {
    let mut guard = MSG_IN_BUF.lock().unwrap();
    loop {
        match guard.pop_back() {
            None => guard = HAS_IN_MSG.wait(guard).unwrap(),
            Some(msg) => {
                return msg;
            }
        }
    }
}

unsafe fn push_msg(msg: Message) {
    let mut guard = MSG_OUT_BUF.lock().unwrap();
    guard.push_front(msg);
    HAS_OUT_MSG.notify_all();
}

unsafe fn worker_thread(table: Arc<HashTable>) {
    loop {
        let msg = pop_msg();
        match msg {
            Message::Insert(k, v) => table.insert(k, v),
            Message::Delete(k) => table.delete(k),
            Message::Read(k) => {
                let value = table.read(k);
                //Push response to Response Queue
                let msg = match value {
                    None => Message::No,
                    Some(v) => Message::Some(v),
                };
                push_msg(msg);
            }
            Message::Print(i) => table.print(i),
            Message::Quit => exit(0),
            Message::ThStop => {
                break;
            }
            Message::ThStart(n) => {
                THREAD_NUM = n;
            }
            _ => continue,
        }
    }
}

