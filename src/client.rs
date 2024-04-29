mod util;
use std::cmp::max;
use std::sync::atomic::AtomicUsize;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use std::time::Instant;

use util::{create_message_field, MessageField, Operation, THREAD_NUM};

const NUM_ITEMS: usize = 1e6 as usize;
const SLICE_SIZE: usize = NUM_ITEMS / THREAD_NUM;
static SEQ: AtomicUsize = AtomicUsize::new(0);

fn main() {
    //establish communication between processes and threads
    let field = create_message_field(false);
    let (sender, receiver) = mpsc::channel();

    //setup for the tests
    let mut res = true;
    let mut dur = Duration::from_micros(0);

    //run tests

    print!("Sequential Insert: ");
    for i in 0..THREAD_NUM {
        let tmp = field as usize;
        let sender_clone = sender.clone();
        thread::spawn(move || {
            let result = test_sequential_values(tmp, i);
            sender_clone.send(result).unwrap();
        });
    }

    //collect results from threads
    for _ in 0..THREAD_NUM {
        let ret = receiver.recv().unwrap();
        res &= ret.0;
        dur = max(dur, ret.1);
    }
    print!("{} in {:.2?}\n", if res { "passed" } else { "failed" }, dur);

    put_work(field, Operation::Quit, 0);
    println!();
}

fn put_work(field: *mut MessageField, op: Operation, id: usize) -> usize {
    unsafe {
        (*field).put_work(
            op,
            SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            id,
        )
    }
}

fn pick_up(field: *mut MessageField, index: usize) -> Operation {
    unsafe { (*field).pick_up_result(index) }
}

//inserts items, checks for their correct insertion, deletes them and then checks they are no longer present.
fn test_sequential_values(field: usize, id: usize) -> (bool, Duration) {
    let now = Instant::now();
    let field = field as *mut MessageField;
    let start_index = id * SLICE_SIZE;
    let end_index = start_index + SLICE_SIZE;

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
            _ => return (false, now.elapsed()),
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
