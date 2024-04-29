mod util;
use std::cmp::max;
use std::sync::atomic::AtomicUsize;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use std::time::Instant;

use libc::rand;
use libc::srand;
use util::{create_message_field, MessageField, Operation, THREAD_NUM};

const NUM_ITEMS: usize = 1e6 as usize;
const SLICE_SIZE: usize = NUM_ITEMS / THREAD_NUM;
static SEQ: AtomicUsize = AtomicUsize::new(0);

fn main() {
    //establish communication between processes and threads
    let field = create_message_field(false);
    let (sender, receiver) = mpsc::channel();

    //run tests
    for i in 0..THREAD_NUM {
        let tmp = field as usize;
        let sender_clone = sender.clone();
        thread::spawn(move || {
            let mut result = test_sequential_values(tmp, i);
            sender_clone.send(result).unwrap();
            result = test_random_values(tmp, i);
            sender_clone.send(result).unwrap();
        });
    }

    //collect results from threads
    let mut res = true;
    let mut dur = Duration::from_micros(0);
    print!("Sequential Test: ");
    for _ in 0..THREAD_NUM {
        let ret = receiver.recv().unwrap();
        res &= ret.0;
        dur = max(dur, ret.1);
    }
    print!("{} in {:.2?}\n", if res { "passed" } else { "failed" }, dur);

    res = true;
    dur = Duration::from_micros(0);
    print!("Random Test: ");
    for _ in 0..THREAD_NUM {
        let ret = receiver.recv().unwrap();
        res &= ret.0;
        dur = max(dur, ret.1);
    }
    print!("{} in {:.2?}\n", if res { "passed" } else { "failed" }, dur);

    put_work(field, Operation::Quit, 0);
    println!();
}

//send work to the server thread with the same id
fn put_work(field: *mut MessageField, op: Operation, id: usize) -> usize {
    unsafe {
        (*field).put_work(
            op,
            SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            id,
        )
    }
}

//pick up the response from the server thread with the same id 
fn pick_up(field: *mut MessageField, id: usize) -> Operation {
    unsafe { (*field).pick_up_result(id) }
}

//inserts items, checks for their correct insertion, deletes them and then checks they are no longer present.
fn test_sequential_values(field: usize, id: usize) -> (bool, Duration) {
    let field = field as *mut MessageField;
    let start_index = id * SLICE_SIZE;
    let end_index = start_index + SLICE_SIZE;
    let now = Instant::now();

    //insert values and check that they are correctly inserted
    for i in start_index..end_index as usize {
        put_work(field, Operation::Insert(i, i * i), id);
        put_work(field, Operation::Read(i), id);
        match pick_up(field, id) {
            Operation::Fail => {
                eprintln!("Missing key-value pair! Fail");
                return (false, now.elapsed());
            }
            Operation::Value(v) => {
                if v != i * i {
                    eprintln!("Wrong Value! Fail");
                    return (false, now.elapsed());
                }
            }
            _ => {
                eprintln!("Unexpected response! Fail!");
                return (false, now.elapsed());
            }
        }
    }

    //delete values and check that they are deleted
    for i in start_index..end_index {
        put_work(field, Operation::Delete(i), id);
        put_work(field, Operation::Read(i), id);
        match pick_up(field, id) {
            Operation::Value(v) => {
                if v == i * i {
                    eprintln!("Value that should have been deleted was present! Fail");
                    return (false, now.elapsed());
                }
            }
            Operation::Fail => {}
            _ => {
                eprintln!("Unexpected response! Fail!");
                return (false, now.elapsed());
            }
        }
    }
    (true, now.elapsed())
}

//insert random values, delete 10 of them, check the deletion and then check if all others are still present
fn test_random_values(field: usize, id: usize) -> (bool, Duration) {
    let field = field as *mut MessageField;
    let now = Instant::now();
    let low_border = id * SLICE_SIZE;
    let high_border = low_border + SLICE_SIZE;
    let mut keys = Vec::new();
    unsafe {
        srand(id as u32);
        for _ in 0..SLICE_SIZE {
            //make sure that the threads are not generating the same numbers
            let key = low_border + (rand() as usize % (high_border - low_border));
            keys.push(key);
            put_work(field, Operation::Insert(key, key / 2), id);
        }

        let mut rand_keys = Vec::new();
        for _ in 0..10 {
            let i = rand() as usize % keys.len();
            rand_keys.push(*keys.get(i).unwrap());
        }
        for key in rand_keys.iter() {
            put_work(field, Operation::Delete(*key), id);
        }
        for key in rand_keys.iter() {
            put_work(field, Operation::Read(*key), id);
            match pick_up(field, id) {
                Operation::Fail => {}
                Operation::Value(v) => {
                    if v == key / 2 {
                        eprintln!("Value that should have been deleted was present! Fail");
                        return (false, now.elapsed());
                    }
                }
                _ => {
                    eprintln!("Unexpected response! Fail!");
                    return (false, now.elapsed());
                }
            }
        }
        for key in keys {
            if rand_keys.contains(&key) {
                continue;
            }
            put_work(field, Operation::Read(key), id);
            match pick_up(field, id) {
                Operation::Fail => {
                    eprintln!("Missing key-value pair! Fail");
                    return (false, now.elapsed());
                }
                Operation::Value(v) => {
                    if v != key / 2 {
                        eprintln!("Wrong Value! Fail");
                        return (false, now.elapsed());
                    }
                }
                _ => {
                    eprintln!("Unexpected response! Fail!");
                    return (false, now.elapsed());
                }
            }
        }
    }
    (true, now.elapsed())
}
