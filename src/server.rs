mod util;

use std::{process::exit, sync::Arc, thread::spawn};

use util::{create_msq, get_size, HashTable, Message, MessageQueue};

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
        let table = hashtable.clone();
        let _ = spawn(move || worker(tmp_in, tmp_out, table)).join();
    }
}

unsafe fn worker(incoming_ptr: usize, outgoing_ptr: usize, table: Arc<HashTable>) {
    let in_ptr = incoming_ptr as *mut MessageQueue;
    let out_ptr = outgoing_ptr as *mut MessageQueue;
    loop {
        let table = table.clone();
        let msg = (*in_ptr).dequeue();
        match msg {
            Message::Insert(k, v) => table.insert(k, v),
            Message::Delete(k) => table.delete(k),
            Message::Read(k) => {
                let value = table.read(k);
                let msg = match value {
                    None => Message::Fail,
                    Some(v) => Message::Value(v),
                };
                (*out_ptr).enqueue(msg);
            }
            Message::Print(i) => table.print(i),
            Message::Quit => exit(0),
            Message::ThStart(n) => {
                for _ in 0..n {
                    let table = table.clone();
                    spawn(move || worker(incoming_ptr, outgoing_ptr, table));
                }
            }
            Message::ThStop => break,
            _ => {}
        }
    }
}
