use std::{fs, thread, time::Duration};
use exponential_backoff::Backoff;

struct Retryer{
    backoff: Backoff,
}

impl Retryer {
    
    pub fn new() -> Self {
        let retries = 8;
        let min = Duration::from_millis(100);
        let max = Duration::from_secs(10);
        let backoff = Backoff::new(retries, min, max);

        Retryer { 
            backoff,
        }
    }

    // pub fn do_with_retry(&self, function: FnOnce<T>) -> ABCDResult<T> {
    //     for duration in &backoff {
    //         match function() {
    //             Ok(t) => return Ok(t),
    //             Err(err) => match duration {
    //                 Some(duration) => thread::sleep(duration),
    //                 None => return err,
    //             }
    //         }
    //     }
    // }
}