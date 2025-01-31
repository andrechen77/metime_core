use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
    sync::{Arc, Mutex},
};

use chrono::{prelude::*, TimeDelta};
use derive_more::{
    derive::{Display, From, TryInto},
    TryIntoError,
};
use uuid::Uuid;

#[derive(Debug)]
pub struct EventInstance {
    pub time_span: TimeSpan,
    pub body: Uuid,
}

#[derive(Debug, Display, PartialEq, Eq)]
pub enum TimeSpan {
    #[display("[{}]", _0.format("%c"))]
    Instant(DateTime<Utc>),
    #[display("[{} -- {}m]", start.format("%c"), duration.num_minutes())]
    Interval {
        start: DateTime<Utc>,
        duration: TimeDelta,
    },
    // TODO add dates and date intervals (without times)
}

#[derive(Debug)]
pub struct EventBody {
    pub summary: String,
    pub description: String,
    // TODO add location, categories, etc.
}

#[derive(Debug)]
pub enum RepoRetrievalError {
    /// The item associated with the ID has already been retrieved. Either use
    /// the existing retrieval or release it back to the repo before retrieving
    /// it again.
    AlreadyRetrieved,
    /// The item associated with the ID could not be found.
    IdNotFound,
    /// The item associated with the ID is of a different type than expected.
    MismatchedType,
}

// TODO explain the concept of "retrieval", which is like a borrow for repo
// data
/// Trait for interacting with some backing repository for retrieving, caching,
/// and modifying application data in-memory.
pub trait Repository {
    type EventInstanceId: Copy;
    type EventBodyId: Copy;

    /// Get the data of an event instance given its ID.
    fn get_event_instance(
        &self,
        id: Self::EventInstanceId,
    ) -> Result<impl DerefMut<Target = EventInstance> + 'static, RepoRetrievalError>;

    /// Adds a new event instance to the repository. Returns the ID of the event
    /// instance and a reference to the data.
    #[must_use]
    fn add_event_instance(
        &mut self,
        instance: EventInstance,
    ) -> (
        Self::EventInstanceId,
        impl DerefMut<Target = EventInstance> + 'static,
    );

    /// Get the data of an event body given its ID.
    fn get_event_body(
        &self,
        id: Self::EventBodyId,
    ) -> Result<impl DerefMut<Target = EventBody> + 'static, RepoRetrievalError>;

    /// Adds a new event body to the repository. Returns the ID of the event
    /// body and a reference to the data.
    #[must_use]
    fn add_event_body(
        &mut self,
        body: EventBody,
    ) -> (
        Self::EventBodyId,
        impl DerefMut<Target = EventBody> + 'static,
    );
}

#[derive(Default)]
pub struct MemoryRepo {
    data: HashMap<Uuid, Arc<Mutex<Option<RepoEntry>>>>,
}

impl std::fmt::Debug for MemoryRepo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut map = f.debug_map();
        for (id, entry) in &self.data {
            use std::sync::TryLockError;
            match entry.try_lock() {
                Ok(entry) => {
                    if let Some(repo_entry) = entry.as_ref() {
                        map.entry(&id, repo_entry);
                    } else {
                        map.entry(&id, &"<retrieved elsewhere>");
                    }
                }
                Err(TryLockError::WouldBlock) => {
                    map.entry(&id, &"<locked>");
                }
                Err(TryLockError::Poisoned(poison_error)) => {
                    map.entry(&id, &&**poison_error.get_ref());
                }
            }
        }
        map.finish()
    }
}

impl MemoryRepo {
    pub fn new() -> Self {
        Self::default()
    }

    fn get_item<T>(&self, id: Uuid) -> Result<RepoRef<T>, RepoRetrievalError>
    where
        Box<T>: TryFrom<RepoEntry, Error = TryIntoError<RepoEntry>> + Into<RepoEntry>,
    {
        // obtain access to the entry
        let entry_ptr = self.data.get(&id).ok_or(RepoRetrievalError::IdNotFound)?;
        let mut entry = entry_ptr.lock().unwrap();

        // make sure the contents exist (i.e. not already retrieved) and are
        // of the right type
        let contents = entry.take().ok_or(RepoRetrievalError::AlreadyRetrieved)?;
        match contents.try_into() {
            Ok(correct_type) => Ok(RepoRef {
                data: Some(correct_type),
                home_slot: entry_ptr.clone(),
            }),
            Err(other_type) => {
                // put the entry back because it was not the expected type
                *entry = Some(other_type.input);
                Err(RepoRetrievalError::MismatchedType)
            }
        }
    }
}

impl Repository for MemoryRepo {
    type EventInstanceId = Uuid;
    type EventBodyId = Uuid;

    fn get_event_instance(
        &self,
        id: Self::EventInstanceId,
    ) -> Result<impl DerefMut<Target = EventInstance> + 'static, RepoRetrievalError> {
        self.get_item(id)
    }

    fn add_event_instance(
        &mut self,
        instance: EventInstance,
    ) -> (
        Self::EventInstanceId,
        impl DerefMut<Target = EventInstance> + 'static,
    ) {
        let id = Uuid::new_v4();

        // construct the entry as empty; the returned reference will fill in the
        // entry when it is dropped
        let entry = Arc::new(Mutex::new(None));
        self.data.insert(id, entry.clone());

        (
            id,
            RepoRef {
                data: Some(Box::new(instance)),
                home_slot: entry,
            },
        )
    }

    fn get_event_body(
        &self,
        id: Self::EventBodyId,
    ) -> Result<impl DerefMut<Target = EventBody> + 'static, RepoRetrievalError> {
        self.get_item(id)
    }

    fn add_event_body(
        &mut self,
        body: EventBody,
    ) -> (
        Self::EventBodyId,
        impl DerefMut<Target = EventBody> + 'static,
    ) {
        let id = Uuid::new_v4();

        // construct the entry as empty; the returned reference will fill in the
        // entry when it is dropped
        let entry = Arc::new(Mutex::new(None));
        self.data.insert(id, entry.clone());

        (
            id,
            RepoRef {
                data: Some(Box::new(body)),
                home_slot: entry,
            },
        )
    }
}

#[derive(Debug, From, TryInto)]
enum RepoEntry {
    EventInstance(Box<EventInstance>),
    EventBody(Box<EventBody>),
}

#[derive(Debug)]
struct RepoRef<T>
where
    Box<T>: Into<RepoEntry>,
{
    // This is only an option so that it can be moved out in the destructor.
    // During normal operation, it can be assumed that this is always `Some`.
    /// The data being referenced.
    data: Option<Box<T>>,
    /// The slot where the data will be returned when this reference is dropped.
    home_slot: Arc<Mutex<Option<RepoEntry>>>,
}

impl<T> Deref for RepoRef<T>
where
    Box<T>: Into<RepoEntry>,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data
            .as_ref()
            .expect("data should be Some in normal operation")
            .as_ref()
    }
}

impl<T> DerefMut for RepoRef<T>
where
    Box<T>: Into<RepoEntry>,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data
            .as_mut()
            .expect("data should be Some in normal operation")
            .as_mut()
    }
}

impl<T> Drop for RepoRef<T>
where
    Box<T>: Into<RepoEntry>,
{
    fn drop(&mut self) {
        let mut home_slot = self.home_slot.lock().unwrap();
        if home_slot.is_some() {
            panic!("RepoRef was dropped but its home slot was already filled");
            // TODO handle more gracefully, such as by doing nothing or
            // replacing the data while emitting a warning
        }

        let data = self
            .data
            .take()
            .expect("data should be Some before the destructor");
        *home_slot = Some(data.into());
    }
}
