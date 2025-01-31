use std::{
    collections::HashMap,
    fmt::Debug,
    ops::{Deref, DerefMut},
    sync::{Arc, Mutex},
};

use derive_more::{
    derive::{From, TryInto},
    TryIntoError,
};
use uuid::Uuid;

use crate::domain::{EventBody, EventInstance, Timeline};

use super::{RepoRetrievalError, Repository};

#[derive(Default, Debug)]
pub struct MemoryRepo {
    timeline: SlotPtr<Box<Timeline<Self>>>,
    blobs: HashMap<Uuid, SlotPtr<Blob>>,
}

impl MemoryRepo {
    pub fn new() -> Self {
        Self::default()
    }

    fn lend_from_blobs<T>(&self, id: Uuid) -> Result<RepoRef<T, Blob>, RepoRetrievalError>
    where
        Box<T>: Into<Blob>,
        Blob: TryInto<Box<T>, Error = TryIntoError<Blob>>,
    {
        let entry_ptr = self
            .blobs
            .get(&id)
            .ok_or(RepoRetrievalError::IdNotFound)?
            .clone();
        lend_item(entry_ptr, |blob| blob.try_into().map_err(|e| e.input))
            .ok_or(RepoRetrievalError::AlreadyRetrieved)
    }
}

impl Repository for MemoryRepo {
    fn get_timeline(&self) -> Option<impl DerefMut<Target = Timeline<Self>> + 'static + use<>> {
        lend_item(self.timeline.clone(), Ok)
    }

    type EventInstanceId = Uuid;

    fn get_event_instance(
        &self,
        id: Self::EventInstanceId,
    ) -> Result<impl DerefMut<Target = EventInstance<Self>> + 'static + use<>, RepoRetrievalError>
    {
        self.lend_from_blobs(id)
    }

    fn add_event_instance(
        &mut self,
        instance: EventInstance<Self>,
    ) -> (
        Self::EventInstanceId,
        impl DerefMut<Target = EventInstance<Self>> + 'static + use<>,
    ) {
        let id = Uuid::new_v4();

        // construct the entry as empty; the returned reference will fill in the
        // entry when it is dropped
        let entry = SlotPtr(Arc::new(Mutex::new(None)));
        self.blobs.insert(id, entry.clone());

        (
            id,
            RepoRef {
                data: Some(Box::new(instance)),
                home_slot: entry,
            },
        )
    }

    type EventBodyId = Uuid;

    fn get_event_body(
        &self,
        id: Self::EventBodyId,
    ) -> Result<impl DerefMut<Target = EventBody> + 'static + use<>, RepoRetrievalError> {
        self.lend_from_blobs(id)
    }

    fn add_event_body(
        &mut self,
        body: EventBody,
    ) -> (
        Self::EventBodyId,
        impl DerefMut<Target = EventBody> + 'static + use<>,
    ) {
        let id = Uuid::new_v4();

        // construct the entry as empty; the returned reference will fill in the
        // entry when it is dropped
        let entry = SlotPtr(Arc::new(Mutex::new(None)));
        self.blobs.insert(id, SlotPtr::clone(&entry));

        (
            id,
            RepoRef {
                data: Some(Box::new(body)),
                home_slot: entry,
            },
        )
    }
}

fn lend_item<T, S, F>(entry_ptr: SlotPtr<S>, convert_item: F) -> Option<RepoRef<T, S>>
where
    Box<T>: Into<S>,
    F: FnOnce(S) -> Result<Box<T>, S>,
{
    // make sure the contents exist (i.e. not already retrieved) and are
    // of the right type
    let mut entry = entry_ptr.0.lock().unwrap();
    let contents = entry.take()?;
    match convert_item(contents) {
        Ok(correct_type) => {
            drop(entry); // end the borrow of entry_ptr
            Some(RepoRef {
                data: Some(correct_type),
                home_slot: entry_ptr,
            })
        }
        Err(other_type) => {
            // put the entry back because it was not the expected type
            *entry = Some(other_type);
            panic!("entry was not the expected type");
        }
    }
}

struct SlotPtr<T>(Arc<Mutex<Option<T>>>);

impl<T> Clone for SlotPtr<T> {
    fn clone(&self) -> Self {
        SlotPtr(Arc::clone(&self.0))
    }
}

impl<T: Default> Default for SlotPtr<T> {
    fn default() -> Self {
        SlotPtr(Arc::new(Mutex::new(Some(T::default()))))
    }
}

impl<T> Debug for SlotPtr<T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let entry = &self.0;

        use std::sync::TryLockError;
        match entry.try_lock() {
            Ok(entry) => {
                if let Some(repo_entry) = entry.as_ref() {
                    repo_entry.fmt(f)
                } else {
                    f.write_str("<retrieved elsewhere>")
                }
            }
            Err(TryLockError::WouldBlock) => f.write_str("<locked>"),
            Err(TryLockError::Poisoned(poison_error)) => poison_error.get_ref().fmt(f),
        }
    }
}

#[derive(Debug, From, TryInto)]
enum Blob {
    EventInstance(Box<EventInstance<MemoryRepo>>),
    EventBody(Box<EventBody>),
}

#[derive(Debug)]
struct RepoRef<T, S>
where
    Box<T>: Into<S>,
{
    // This is only an option so that it can be moved out in the destructor.
    // During normal operation, it can be assumed that this is always `Some`.
    /// The data being referenced.
    data: Option<Box<T>>,
    /// The slot where the data will be returned when this reference is dropped.
    home_slot: SlotPtr<S>,
}

impl<T, S> Deref for RepoRef<T, S>
where
    Box<T>: Into<S>,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data
            .as_ref()
            .expect("data should be Some in normal operation")
            .as_ref()
    }
}

impl<T, S> DerefMut for RepoRef<T, S>
where
    Box<T>: Into<S>,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data
            .as_mut()
            .expect("data should be Some in normal operation")
            .as_mut()
    }
}

impl<T, S> Drop for RepoRef<T, S>
where
    Box<T>: Into<S>,
{
    fn drop(&mut self) {
        let mut home_slot = self.home_slot.0.lock().unwrap();
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
