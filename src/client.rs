mod util;
use std::collections::VecDeque;
use std::sync::Condvar;
use std::sync::Mutex;
use std::thread::spawn;
use std::time::Duration;
use std::time::Instant;

use libc::rand;
use util::Message;
use util::MessageQueue;

use util::create_msq;

const NUM_ITEM: usize = 1e6 as usize;

static mut MSG_IN_BUF: Mutex<VecDeque<Message>> = Mutex::new(VecDeque::new()); //buffers incoming messages
static mut HAS_IN_MSG: Condvar = Condvar::new();

static mut MSG_OUT_BUF: Mutex<VecDeque<Message>> = Mutex::new(VecDeque::new()); //buffers outgoing messages
static mut HAS_OUT_MSG: Condvar = Condvar::new();

fn main() {
    unsafe {
        let outgoing = create_msq(false, true);
        let incoming = create_msq(false, false);

        let tmp_in = incoming as usize;
        let tmp_out = outgoing as usize;
        spawn(move || in_message_thread(tmp_in));
        spawn(move || out_message_thread(tmp_out));
        let mut n = 1;
        println!();
        //Run the tests for multiple thread numbers
        let mut res;
        while n <= 16 {
            println!("Running tests using {} threads: ", n);
            print!("Sequenteial Insert: ");
            res = test_sequential_insert();
            print!(
                "{} in {:.2?}\n",
                if res.0 { "passed" } else { "failed" },
                res.1
            );
            // print!("Random insert: ");
            // res = test_random_ri(address, response);
            // print!(
            //     "{} in {:.2?}\n",
            //     if res.0 { "passed" } else { "failed" },
            //     res.1
            // );
            //stop and restart the threads
            (*outgoing).enqueue(Message::ThStart(n * 2));
            let msg = Message::ThStop;
            for _ in 0..n {
                (*outgoing).enqueue(msg);
            }
            n *= 2;
            println!();
        }
        (*outgoing).enqueue(Message::Quit);
    }
}

//inserts NUM_ITEMs sequential values, checks if they are present and removes them
unsafe fn test_sequential_insert() -> (bool, Duration) {
    let now = Instant::now();

    for i in 0..NUM_ITEM {
        push_msg(Message::Insert(i, i * i));
    }

    for i in 0..NUM_ITEM {
        push_msg(Message::Read(i));
        let msg = pop_msg();
        match msg {
            Message::No => {
                eprintln!("Value not present! Fail");
                return (false, now.elapsed());
            }
            _ => {}
        }
    }
    for i in 0..NUM_ITEM {
        push_msg(Message::Delete(i))
    }
    (true, now.elapsed())
}

//inserts NUM_ITEMs random key value pairs, checks if they are present and removes them
unsafe fn test_random_ri(
    address: *mut MessageQueue,
    response: *mut MessageQueue,
) -> (bool, Duration) {
    let mut keys = Vec::new();
    let now = Instant::now();

    for _ in 0..NUM_ITEM {
        let key = rand() as usize;
        keys.push(key);
        push_msg(Message::Insert(key, rand() as usize));
    }

    for key in keys.iter() {
        push_msg(Message::Read(*key));
        let msg = (*response).dequeue();
        match msg {
            Message::No => {
                eprintln!("Value not present! Fail");
                return (false, now.elapsed());
            }
            _ => {}
        }
    }

    for key in keys {
        (*address).enqueue(Message::Delete(key));
    }

    (true, now.elapsed())
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
    HAS_OUT_MSG.notify_one();
}

unsafe fn in_message_thread(in_ptr: usize) {
    let in_ptr = in_ptr as *mut MessageQueue;
    loop {
        let msg = (*in_ptr).dequeue();
        let mut guard = MSG_IN_BUF.lock().unwrap();
        guard.push_front(msg);
        HAS_IN_MSG.notify_one();
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
