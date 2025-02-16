use std::{any::TypeId, cell::RefCell, rc::Rc};

use bevy_reflect::Reflect;
use bevy_utils::hashbrown::{HashMap, HashSet};

use crate::{systems::SystemId, world::World};

Component! {
    pub(crate) struct CurrentSystemId {
        #[educe(Debug(ignore))]
        pub(crate) value: SystemId,
    }
}

pub trait Event: 'static {}
impl<T: 'static> Event for T {}

impl CurrentSystemId {
    pub(crate) fn new(value: SystemId) -> Self {
        Self { value }
    }
}

pub(crate) fn default_cleanup_system<T: Event>(world: &World) {
    world.resources::<&mut Events<T>>(|events| {
        events.update();
    });
}

pub struct EventReader<T: Event> {
    data: std::marker::PhantomData<T>,
    read_ids: Rc<RefCell<HashSet<EventId>>>,
    events: Rc<RefCell<Vec<EventData<T>>>>,
}

pub struct EventIter<'w, T: Event> {
    events: &'w Rc<RefCell<Vec<EventData<T>>>>,
    read_ids: &'w Rc<RefCell<HashSet<EventId>>>,
    index: usize,
}

impl<'w, T: Event> Iterator for EventIter<'w, T> {
    type Item = &'w T;

    fn next(&mut self) -> Option<Self::Item> {
        let events = self.events.borrow();
        let mut read_ids = self.read_ids.borrow_mut();
        let event_data = loop {
            if self.index == events.len() {
                return None;
            }

            let event_data = &events[self.index];
            if read_ids.contains(&event_data.id) {
                self.index += 1;
                continue;
            }

            read_ids.insert(event_data.id);
            break event_data;
        };
        Some(unsafe { &*(&event_data.event as *const T) })
    }
}

impl<T: Event> EventReader<T> {
    pub fn new(events: Rc<RefCell<Vec<EventData<T>>>>) -> Self {
        Self {
            data: std::marker::PhantomData,
            read_ids: RefCell::new(HashSet::new()).into(),
            events,
        }
    }

    pub fn read(&self) -> EventIter<'_, T> {
        EventIter {
            events: &self.events,
            index: 0,
            read_ids: &self.read_ids,
        }
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct EventId(pub u64);

pub struct Events<T: Event> {
    events: Rc<RefCell<Vec<EventData<T>>>>,
    readers: HashMap<SystemId, Rc<RefCell<EventReader<T>>>>,
    last_id: EventId,
}
///Fresh: given to an event upon creation
///Dirty: assigned to an event when it reaches a cleanup system at the end of a frame
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EventState {
    Fresh,
    Dirty,
}

pub struct EventData<T> {
    event: T,
    id: EventId,
    state: EventState,
}

impl<T> EventData<T> {
    pub fn new(event: T, id: EventId) -> Self {
        Self {
            event,
            id,
            state: EventState::Fresh,
        }
    }
}

impl<T: Event> Events<T> {
    pub fn new() -> Self {
        Self {
            events: RefCell::new(vec![]).into(),
            readers: HashMap::new(),
            last_id: EventId(0),
        }
    }

    pub fn clear(&mut self) {
        self.events.borrow_mut().clear();
    }

    ///Performs event double buffering, so that events are removed by the end of the next frame
    pub fn update(&mut self) {
        let mut events = self.events.borrow_mut();
        self.readers.values().for_each(|reader| {
            events
                .iter()
                .filter(|e| e.state == EventState::Dirty)
                .for_each(|event| {
                    reader.borrow_mut().read_ids.borrow_mut().remove(&event.id);
                })
        });
        events.retain(|e| e.state == EventState::Fresh);
        events.iter_mut().for_each(|e| {
            e.state = EventState::Dirty;
        });
    }

    pub fn push(&mut self, event: T) {
        let id = self.next_id();
        self.events.borrow_mut().push(EventData::new(event, id));
    }

    pub fn next_id(&mut self) -> EventId {
        let id = self.last_id;
        self.last_id.0 = self.last_id.0.wrapping_add(1);
        id
    }

    // pub fn iter_events(&mut self, system_id: TypeId) {
    //     let mut readers = self.readers.borrow_mut();
    //     let reader = readers.entry(system_id).or_insert(EventReader::<T> {
    //         data: Default::default(),
    //         read_ids: RefCell::new(HashSet::new()).into(),
    //     });
    //     reader.iter(&self.events);
    // }

    pub fn event_reader(&mut self, system_id: SystemId) -> Rc<RefCell<EventReader<T>>> {
        self.readers
            .entry(system_id)
            .or_insert(RefCell::new(EventReader::new(self.events.clone())).into())
            .clone()
    }
}
