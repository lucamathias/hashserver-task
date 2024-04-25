mod util;

use std::time::Duration;
use std::time::Instant;

use util::Message;
use util::MessageQueue;

use util::create_msq;

const NUM_ITEM: usize = 1e6 as usize;

fn main() {
    unsafe {
        let outgoing = create_msq(false, true);
        let incoming = create_msq(false, false);

        let mut n = 1;
        println!();
        //Run the tests for multiple thread numbers
        let mut res;
        while n <= 16 {
            println!("Running tests using {} threads: ", n);
            print!("Sequenteial Insert: ");
            res = test_sequential_insert(outgoing, incoming);
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

            //start new threads
            (*outgoing).enqueue(Message::ThStart(n));
            n *= 2;
            println!();
        }
        (*outgoing).enqueue(Message::Quit);
    }
}

//inserts NUM_ITEMs sequential values, checks if they are present and removes them
unsafe fn test_sequential_insert(
    outgoing: *mut MessageQueue,
    incoming: *mut MessageQueue,
) -> (bool, Duration) {
    let now = Instant::now();

    for i in 0..NUM_ITEM {
        (*outgoing).enqueue(Message::Insert(i, i * i));
    }

    for i in 0..NUM_ITEM {
        (*outgoing).enqueue(Message::Read(i));
        let msg = (*incoming).dequeue();
        match msg {
            Message::No => {
                eprintln!("Value not present! Fail");
                return (false, now.elapsed());
            }
            _ => {}
        }
    }

    for i in 0..NUM_ITEM {
        (*outgoing).enqueue(Message::Delete(i))
    }
    (true, now.elapsed())
}

//inserts NUM_ITEMs random key value pairs, checks if they are present and removes them
// unsafe fn test_random_ri(
//     address: *mut MessageQueue,
//     response: *mut MessageQueue,
// ) -> (bool, Duration) {
//     let mut keys = Vec::new();
//     let now = Instant::now();

//     for _ in 0..NUM_ITEM {
//         let key = rand() as usize;
//         keys.push(key);
//         (*outgoing ).enqueue(Message::Insert(key, rand() as usize));
//     }

//     for key in keys.iter() {
//         (*outgoing ).enqueue(Message::Read(*key));
//         let msg = (*response).dequeue();
//         match msg {
//             Message::No => {
//                 eprintln!("Value not present! Fail");
//                 return (false, now.elapsed());
//             }
//             _ => {}
//         }
//     }

//     for key in keys {
//         (*address).enqueue(Message::Delete(key));
//     }

//     (true, now.elapsed())
// }
