mod util;
use std::sync::atomic::AtomicUsize;
use std::time::Duration;
use std::time::Instant;

use libc::rand;
use libc::srand;

use util::{create_message_field, MessageField, Operation};

const NUM_ITEMS: usize = 2e6 as usize;
static SEQ: AtomicUsize = AtomicUsize::new(0);

fn main() {
    let field = create_message_field(false);
    let mut res;
    print!("Sequential Insert: ");
    res = test_sequential_i(field);
    print!(
        "{} in {:.2?}\n",
        if res.0 { "passed" } else { "failed" },
        res.1
    );
    print!("Sequential Read: ");
    res = test_sequential_r(field);
    print!(
        "{} in {:.2?}\n",
        if res.0 { "passed" } else { "failed" },
        res.1
    );
    print!("Sequential Delete: ");
    res = test_sequential_d(field);
    print!(
        "{} in {:.2?}\n",
        if res.0 { "passed" } else { "failed" },
        res.1
    );
    print!("Random insert check delete: ");
    res = test_random_ird(field);
    print!(
        "{} in {:.2?}\n",
        if res.0 { "passed" } else { "failed" },
        res.1
    );
    put_work(field, Operation::Quit);
    println!();
}

fn put_work(field: *mut MessageField, op: Operation) -> usize {
    unsafe { (*field).put_work(op, SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)) }
}

fn pick_up(field: *mut MessageField, index: usize) -> Operation {
    unsafe { (*field).pick_up_result(index) }
}

fn test_sequential_i(field: *mut MessageField) -> (bool, Duration) {
    let now = Instant::now();
    for i in 0..NUM_ITEMS {
        put_work(field, Operation::Insert(i, i * i));
        let index = put_work(field, Operation::Read(i));
        match pick_up(field, index) {
            Operation::Fail => {
                eprintln!("Value not present! Fail");
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
    (true, now.elapsed())
}

// //checks for the presence of the inserted values sequentially
fn test_sequential_r(field: *mut MessageField) -> (bool, Duration) {
    let now = Instant::now();
    for i in 0..NUM_ITEMS {
        let index = put_work(field, Operation::Read(i));
        match pick_up(field, index) {
            Operation::Fail => {
                eprintln!("Value not present! Fail");
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
    (true, now.elapsed())
}

//Delets the previously inserted values sequentially
fn test_sequential_d(field: *mut MessageField) -> (bool, Duration) {
    let now = Instant::now();
    for i in 0..NUM_ITEMS {
        put_work(field, Operation::Delete(i));
    }
    (true, now.elapsed())
}

fn test_random_ird(field: *mut MessageField) -> (bool, Duration) {
    let mut keys = Vec::new();
    let now = Instant::now();

    for _ in 0..NUM_ITEMS {
        let key = unsafe { rand() } as usize;
        keys.push(key);
        put_work(field, Operation::Insert(key, key / 2)); //value always key/2
    }

    let mut rand_indices = Vec::new();
    for i in 0..10 {
        unsafe { srand(i) };
        rand_indices.push(unsafe { rand() } as usize);
    }

    //lookup 10 random keys and check values
    for ri in rand_indices {
        let index = ri % keys.len();
        let key = *keys.get(index).unwrap();
        let index = put_work(field, Operation::Read(key));
        match pick_up(field, index) {
            Operation::Fail => {
                eprintln!("Value not present! Fail");
                return (false, now.elapsed());
            }
            Operation::Value(v) => {
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
        unsafe { srand(i) };
        rand_indices.push(unsafe { rand() } as usize);
    }

    //Delete 10 random keys and check that they are not present anymore
    for ri in rand_indices {
        let index = ri % keys.len();
        let key = *keys.get(index).unwrap();
        put_work(field, Operation::Delete(key));
        let index = put_work(field, Operation::Read(key));
        match pick_up(field, index) {
            Operation::Value(v) => {
                if v != key / 2 {
                    eprintln!("Value still present! Fail");
                    return (false, now.elapsed());
                }
            }
            Operation::Fail => {}
            _ => return (false, now.elapsed()),
        }
    }
    (true, now.elapsed())
}
