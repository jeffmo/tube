//use std::collections::HashSet;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;

#[derive(Debug)]
pub enum UniqueIdError {
    NoIdsAvailable,
}

#[derive(Debug)]
pub enum UniqueIdIncrementPolicy {
    Even,
    Odd,
    Sequential,
}

#[derive(Debug)]
pub struct UniqueId {
    avail_ids: Option<Arc<Mutex<VecDeque<u16>>>>,
    id: u16,
    taken: bool,
}
impl UniqueId {
    pub fn new(id: u16, avail_ids: Option<Arc<Mutex<VecDeque<u16>>>>) -> Self {
        UniqueId {
            avail_ids,
            id,
            taken: false,
        }
    }

    /**
     * Be super careful with this.
     *
     * Calling this method will mark this UniqueId so that it's drop (RAII) 
     * logic does not run and will return a new UniqueId object that takes over
     * that RAII cleanup work.
     *
     * This was implemented to address the case where a Tube object owns a 
     * UniqueId object, needs to tokio::spawn() some cleanup work within the
     * Tube::drop() method, and we need to move the UniqueId object into the
     * spawn() lambda. Since you can't really move a field off of `self`, I 
     * couldn't think of any way to preserve the self.tube_id object without b
     * oxing it in some way or another -- but that becomes ergonomically 
     * annoying for all the other places in code that need to access 
     * self.tube_id.
     */
    pub fn take(&mut self) -> Self {
        let new = UniqueId::new(self.id, self.avail_ids.clone());
        self.taken = true;
        new
    }

    pub fn val(&self) -> u16 {
        self.id
    }
}
impl std::fmt::Display for UniqueId {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.id)
    }
}
impl Drop for UniqueId {
    fn drop(&mut self) {
        if !self.taken {
            if let Some(avail_ids) = &self.avail_ids {
                let mut avail_ids = avail_ids.lock().unwrap();
                log::trace!("Marking id={} as available again!", self.id);
                avail_ids.push_back(self.id);
            }
        }
    }
}
impl PartialEq for UniqueId {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

#[derive(Debug)]
pub struct UniqueIdManager { 
    avail_ids: Arc<Mutex<VecDeque<u16>>>,
    counter: u16,
    ids_exhausted: bool,
    increment_policy: UniqueIdIncrementPolicy,
}
impl UniqueIdManager {
    fn new_impl(increment_policy: UniqueIdIncrementPolicy) -> Self {
        UniqueIdManager { 
            avail_ids: Arc::new(Mutex::new(VecDeque::new())),
            counter: match increment_policy {
                UniqueIdIncrementPolicy::Even => 0,
                UniqueIdIncrementPolicy::Odd => 1,
                UniqueIdIncrementPolicy::Sequential => 0,
            },
            ids_exhausted: false,
            increment_policy,
        }
    }

    pub fn new() -> Self {
        UniqueIdManager::new_impl(UniqueIdIncrementPolicy::Sequential)
    }

    pub fn new_with_even_ids() -> Self {
        UniqueIdManager::new_impl(UniqueIdIncrementPolicy::Even)
    }

    pub fn new_with_odd_ids() -> Self {
        UniqueIdManager::new_impl(UniqueIdIncrementPolicy::Odd)
    }

    pub fn take_id(&mut self) -> Result<UniqueId, UniqueIdError> {
        // TODO: This implementation will grow the avail_ids vec up to 
        //       ~65k (2^16) if a large number of ids are taken without being
        //       returned fast enough.
        //
        //       This is probably fine for now, but we could probably do a 
        //       little better with a heap of range structs (or something 
        //       like that) for tracking available_ids.
        let mut avail_ids = self.avail_ids.lock().unwrap();
        let id = match avail_ids.pop_front() {
            Some(id) => id,
            None => {
                if self.ids_exhausted {
                    return Err(UniqueIdError::NoIdsAvailable);
                }

                let id = self.counter;
                if id == u16::MAX {
                    self.ids_exhausted = true;
                } else {
                    match self.increment_policy {
                        UniqueIdIncrementPolicy::Even
                        | UniqueIdIncrementPolicy::Odd =>
                            self.counter += 2,

                        UniqueIdIncrementPolicy::Sequential =>
                            self.counter += 1,
                    }
                }
                id
            }
        };

        Ok(UniqueId::new(
            id,
            Some(self.avail_ids.clone()),
        ))
    }
}

#[cfg(test)]
mod uniqueidmanager_tests {
    use super::*;

    #[test]
    fn does_not_emit_same_id_twice() {
        let mut idman = UniqueIdManager::new();
        assert_ne!(idman.take_id().unwrap(), idman.take_id().unwrap());
    }

    #[test]
    fn errors_when_all_ids_exhausted() {
        let mut idman = UniqueIdManager::new();
        let mut ids = vec![];
        for _i in 0..=u16::MAX {
            ids.push(idman.take_id());
        }
        match idman.take_id() {
            Err(UniqueIdError::NoIdsAvailable) => (),
            Ok(_) => assert!(
                false,
                "UniqueIdError did not error when all ids were exhausted!",
            ),
        }
    }

    #[test]
    fn reuses_ids_after_they_are_dropped() {
        let mut idman = UniqueIdManager::new();
        let _id1 = idman.take_id().unwrap();
        let (id2_val, _id3) = {
            let id = idman.take_id().unwrap();
            (id.val(), idman.take_id().unwrap())
        };
        let id4 = idman.take_id().unwrap();
        assert_eq!(id2_val, id4.val());
    }

    #[test]
    fn only_emits_even_ids_for_even_policy() {
        let mut idman = UniqueIdManager::new_with_even_ids();
        let id1 = idman.take_id().unwrap();
        let id2 = idman.take_id().unwrap();
        let id3 = idman.take_id().unwrap();
        assert_eq!(id2.val() % 2, 0);
        assert_eq!(id1.val() + 2, id2.val());
        assert_eq!(id2.val() + 2, id3.val());
    }

    #[test]
    fn only_emits_odd_ids_for_odd_policy() {
        let mut idman = UniqueIdManager::new_with_odd_ids();
        let id1 = idman.take_id().unwrap();
        let id2 = idman.take_id().unwrap();
        let id3 = idman.take_id().unwrap();
        assert_eq!(id2.val() % 2, 1);
        assert_eq!(id1.val() + 2, id2.val());
        assert_eq!(id2.val() + 2, id3.val());
    }

    #[test]
    fn only_emits_sequential_ids_for_sequential_policy() {
        let mut idman = UniqueIdManager::new();
        let id1 = idman.take_id().unwrap();
        let id2 = idman.take_id().unwrap();
        let id3 = idman.take_id().unwrap();
        assert_eq!(id1.val(), 0);
        assert_eq!(id1.val() + 1, id2.val());
        assert_eq!(id2.val() + 1, id3.val());
    }
}
