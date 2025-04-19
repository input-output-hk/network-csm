use std::{collections::HashMap, sync::Arc};

use crate::Id;

#[derive(Clone)]
pub struct ChannelsMap<T> {
    map: Arc<HashMap<Id, T>>,
}

pub struct ChannelsMapBuilder<T> {
    highest: Id,
    map: HashMap<Id, T>,
}

#[derive(Debug, thiserror::Error)]
#[error("Duplicated channel {0:?}")]
pub struct DuplicateChannel(pub Id);

impl<T> ChannelsMapBuilder<T> {
    pub fn new() -> Self {
        let map = HashMap::default();
        Self {
            highest: Id::new(0),
            map,
        }
    }

    pub fn add(&mut self, channel_id: Id, channel: T) -> Result<(), DuplicateChannel> {
        if let Some(_) = self.map.insert(channel_id, channel) {
            Err(DuplicateChannel(channel_id))
        } else {
            self.highest = self.highest.max(channel_id);
            Ok(())
        }
    }

    pub fn has(&self, channel_id: Id) -> bool {
        self.map.contains_key(&channel_id)
    }

    pub fn finalize(self) -> ChannelsMap<T> {
        if self.map.is_empty() {
            panic!("cannot finalize without any channel")
        }
        ChannelsMap {
            map: Arc::new(self.map),
        }
    }
}

impl<T: Clone> ChannelsMap<T> {
    pub fn has_channel(&self, channel_id: Id) -> bool {
        self.map.contains_key(&channel_id)
    }

    pub fn dispatch(&self, channel_id: Id) -> Option<&T> {
        self.map.get(&channel_id)
    }

    pub fn iterate(&self) -> impl Iterator<Item = (&Id, &T)> {
        self.map.iter()
    }

    pub fn map<F, U>(&self, f: F) -> ChannelsMap<U>
    where
        F: Fn(&T) -> U,
    {
        ChannelsMap {
            map: Arc::new(HashMap::from_iter(
                self.map.iter().map(|(id, c)| (*id, f(c))),
            )),
        }
    }
}
