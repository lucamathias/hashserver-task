mod util;

use std::{process::exit, sync::Arc, thread::spawn};

use util::{create_msq, HashTable};

use crate::util::{destroy_shm, MessageQueue, Operation};

const DEFAULT_SIZE: usize = 15;
const THREAD_NUM: usize = 4;

fn main() {
    let size = get_size();
    let hashtable = Arc::new(HashTable::new(size));
    println!(
        "HashTable of size {} created! Waiting for requests...",
        size
    );
    unsafe {
        let address = create_msq(true);
        let mut threads = Vec::new();
        let tmp = address as usize;
        for _ in 0..THREAD_NUM {
            let table = hashtable.clone();
            threads.push(spawn(move || {
                let ptr = tmp as *mut MessageQueue;
                loop {
                    let msg = (*ptr).dequeue();
                    match msg {
                        Operation::Insert(k, v) => table.insert(k, v),
                        Operation::Delete(k) => table.delete(k),
                        Operation::Read(k) => {
                            let value = table.read(k);
                            match value {
                                None => println!("Entry not present in table"),
                                Some(v) => println!("The value belonging to {} is {}", k, v),
                            }
                        }
                        Operation::Print(i) => table.print(i),
                        Operation::Quit => {
                            destroy_shm();
                            exit(0)
                        }
                        _ => continue,
                    }
                }
            }));
        }

        for thread in threads {
            thread.join().unwrap();
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
