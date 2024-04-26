mod util;

use std::collections::VecDeque;
use std::sync::Condvar;
use std::sync::Mutex;
use std::thread::spawn;
use std::time::Duration;
use std::time::Instant;

use libc::rand;
use libc::srand;
use util::Message;
use util::MessageQueue;

use util::create_msq;

const NUM_ITEMS: usize = 1e6 as usize;

static mut IN_BUF: Mutex<VecDeque<Message>> = Mutex::new(VecDeque::new());
static HAS_IN: Condvar = Condvar::new();

fn main() {
    unsafe {
        let outgoing = create_msq(false, true);
        let incoming = create_msq(false, false);

        //spawn one thread to dequeue responses, so that the main thread wont have to wait
        let tmp = incoming as usize;
        spawn(move || buffer_responses(tmp));

        let mut n = 1;
        println!();
        //Run the tests for multiple thread numbers
        let mut res;
        while n <= 16 {
            println!("Running tests using {} thread(s): ", n);
            print!("Sequential Insert: ");
            res = test_sequential_i(outgoing);
            print!(
                "{} in {:.2?}\n",
                if res.0 { "passed" } else { "failed" },
                res.1
            );
            print!("Sequenteial Read: ");
            res = test_sequential_r(outgoing);
            print!(
                "{} in {:.2?}\n",
                if res.0 { "passed" } else { "failed" },
                res.1
            );
            print!("Sequential Delete: ");
            res = test_sequential_d(outgoing);
            print!(
                "{} in {:.2?}\n",
                if res.0 { "passed" } else { "failed" },
                res.1
            );
            print!("Random Insert, Read, Delete: ");
            res = test_random_ird(outgoing);
            print!(
                "{} in {:.2?}\n",
                if res.0 { "passed" } else { "failed" },
                res.1
            );
            //start new threads
            (*outgoing).enqueue(Message::ThStart(n));
            n *= 2;
            println!();
        }
        (*outgoing).enqueue(Message::Quit);
    }
}

//inserts NUM_ITEMS sequentially
unsafe fn test_sequential_i(outgoing: *mut MessageQueue) -> (bool, Duration) {
    let now = Instant::now();
    for i in 0..NUM_ITEMS {
        (*outgoing).enqueue(Message::Insert(i, i * i));
    }
    (true, now.elapsed())
}

//checks for the presence of the inserted values sequentially
unsafe fn test_sequential_r(outgoing: *mut MessageQueue) -> (bool, Duration) {
    let now = Instant::now();

    for i in 0..NUM_ITEMS {
        (*outgoing).enqueue(Message::Read(i));
        match pop_msg() {
            Message::Fail => {
                eprintln!("Value not present! Fail");
                return (false, now.elapsed());
            }
            Message::Value(v) => {
                if v != i * i {
                    eprintln!("Wrong Value! Fail");
                    return (false, now.elapsed());
                }
            }
            _ => return (false, now.elapsed()),
        }
    }
    (true, now.elapsed())
}

//Delets the previously inserted values sequentially
unsafe fn test_sequential_d(outgoing: *mut MessageQueue) -> (bool, Duration) {
    let now = Instant::now();
    for i in 0..NUM_ITEMS {
        (*outgoing).enqueue(Message::Delete(i));
    }
    (true, now.elapsed())
}

//inserts NUM_TO_INSERT random key value pairs, chooses 10 at random and checks if they are correctly stored.
//Then deletes 10 at random and checks if they are no longer present
unsafe fn test_random_ird(outgoing: *mut MessageQueue) -> (bool, Duration) {
    let mut keys = Vec::new();
    let now = Instant::now();

    for _ in 0..NUM_ITEMS {
        let key = rand() as usize;
        keys.push(key);
        (*outgoing).enqueue(Message::Insert(key, key / 2)); //value always key/2
    }

    let mut rand_indices = Vec::new();
    for i in 0..10 {
        srand(i);
        rand_indices.push(rand() as usize);
    }

    //lookup 10 random keys and check values
    for ri in rand_indices {
        let index = ri % keys.len();
        let key = *keys.get(index).unwrap();
        (*outgoing).enqueue(Message::Read(key));
        match pop_msg() {
            Message::Fail => {
                eprintln!("Value not present! Fail");
                return (false, now.elapsed());
            }
            Message::Value(v) => {
                if v != key / 2 {
                    eprintln!("Wrong Value! Fail");
                    return (false, now.elapsed());
                }
            }
            _ => return (false, now.elapsed()),
        }
    }

    let mut rand_indices = Vec::new();
    for i in 0..10 {
        srand(i);
        rand_indices.push(rand() as usize);
    }

    //Delete 10 random keys and check that they are not present anymore
    for ri in rand_indices {
        let index = ri % keys.len();
        let key = *keys.get(index).unwrap();
        (*outgoing).enqueue(Message::Delete(key));
        (*outgoing).enqueue(Message::Read(key));
        match pop_msg() {
            Message::Value(v) => {
                if v != key / 2 {
                    eprintln!("Value still present! Fail");
                    return (false, now.elapsed());
                }
            }
            Message::Fail => {}
            _ => return (false, now.elapsed()),
        }
    }
    (true, now.elapsed())
}

unsafe fn pop_msg() -> Message {
    let mut guard = IN_BUF.lock().unwrap();
    loop {
        match guard.pop_front() {
            None => guard = HAS_IN.wait(guard).unwrap(),
            Some(msg) => {
                break msg;
            }
        }
    }
}

unsafe fn buffer_responses(incoming: usize) {
    let incoming = incoming as *mut MessageQueue;
    loop {
        let msg = (*incoming).dequeue();
        let mut guard = IN_BUF.lock().unwrap();
        guard.push_back(msg);
        HAS_IN.notify_one();
    }
}
