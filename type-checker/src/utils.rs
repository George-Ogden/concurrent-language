use counter::Counter;
use itertools::Itertools;
use std::{
    fmt::{Debug, Display},
    hash::Hash,
};

pub struct UniqueError<T: Debug> {
    pub duplicate: T,
}

pub fn check_unique<I, T>(items: I) -> Result<(), UniqueError<T>>
where
    I: Iterator<Item = T> + Clone,
    T: Eq + Hash + Display + Debug,
{
    if !items.clone().all_unique() {
        let item_counts = items.collect::<Counter<_>>();
        for (item, count) in item_counts {
            if count > 1 {
                return Err(UniqueError { duplicate: item });
            }
        }
        panic!("Items were not unique but all counts were < 2");
    }
    Ok(())
}
