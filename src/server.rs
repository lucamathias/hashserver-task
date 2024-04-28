mod util;

use std::{sync::Arc, thread::spawn};

use util::{create_message_field, HashTable, MessageField, THREAD_NUM};

const DEFAULT_TABLE_SIZE: usize = 5000;

fn main() {
    let size = get_size();
    let hashtable = Arc::new(HashTable::new(size));
    println!(
        "HashTable of size {} created! Waiting for requests...",
        size
    );
    let msg_field = create_message_field(true);
    let tmp = msg_field as usize;
    let mut threads = Vec::new();

    for i in 0..THREAD_NUM {
        let table = hashtable.clone();
        threads.push(spawn(move || worker(tmp, table, i)));
    }

    for thread in threads {
        thread.join().unwrap();
    }
}

fn worker(field: usize, table: Arc<HashTable>, id: usize) {
    let field = field as *mut MessageField;
    let ht = table.clone();
    loop {
        let ht = ht.clone();
        unsafe {
            (*field).get_work(ht, id);
        }
    }
}
//recieves the size input from the user
fn get_size() -> usize {
    let mut input = String::new();
    println!(
        "Enter the size of the Hashmap (default: {}):",
        DEFAULT_TABLE_SIZE
    );
    std::io::stdin()
        .read_line(&mut input)
        .expect("failed to read line");
    match input.trim().parse::<usize>() {
        Ok(size) => size,
        Err(_) => {
            eprintln!("Invalid input, defaulting to {}", DEFAULT_TABLE_SIZE);
            DEFAULT_TABLE_SIZE
        }
    }
}
