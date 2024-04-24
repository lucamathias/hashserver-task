mod util;
use util::Operation;

use crate::util::create_msq;

const NUMBERS_TO_INSERT: usize = 1e6 as usize;
const 

fn main() {
    unsafe {
        let address = create_msq(false);
        for i in 1..1000000 as usize {
            let msg = Operation::Insert(i, i * i);
            (*address).enqueue(msg);
        }
        for i in 0..3 as usize {
            let msg = Operation::Print(i);
            (*address).enqueue(msg);
        }
    }
}
