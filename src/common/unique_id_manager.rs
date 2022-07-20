//use std::collections::HashSet;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;

#[derive(Debug)]
pub enum UniqueIdError {
    NoIdsAvailable,
}

#[derive(Debug)]
pub struct UniqueId {
    avail_ids: Arc<Mutex<VecDeque<u16>>>,
    id: u16,
}
impl UniqueId {
    pub fn val(&self) -> u16 {
        self.id
    }
}
impl Drop for UniqueId {
    fn drop(&mut self) {
        let mut avail_ids = self.avail_ids.lock().unwrap();
        avail_ids.push_back(self.id);
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
}
impl UniqueIdManager {
    pub fn new() -> Self {
        UniqueIdManager { 
            avail_ids: Arc::new(Mutex::new(VecDeque::new())),
            counter: 0,
            ids_exhausted: false,
        }
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
                    self.counter += 1;
                }
                id
            }
        };

        Ok(UniqueId {
            avail_ids: self.avail_ids.clone(),
            id,
        })
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
        for n in 0..=u16::MAX {
            ids.push(idman.take_id());
        }
        match idman.take_id() {
            Err(UniqueIdError::NoIdsAvailable) => (),
            Ok(unexpected) => assert!(
                false,
                "UniqueIdError did not error when all ids were exhausted!",
            ),
        }
    }

    #[test]
    fn reuses_ids_after_they_are_dropped() {
        let mut idman = UniqueIdManager::new();
        let id1 = idman.take_id().unwrap();
        let (id2_val, id3) = {
            let id = idman.take_id().unwrap();
            (id.val(), idman.take_id().unwrap())
        };
        let id4 = idman.take_id().unwrap();
        assert_eq!(id2_val, id4.val());
    }
}
