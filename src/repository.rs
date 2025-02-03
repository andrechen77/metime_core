use std::ops::DerefMut;

use crate::domain::{EventBody, EventInstance, Timeline};

pub mod memory_repo;

// TODO explain the concept of "retrieval", which is like a borrow for repo
// data
/// Trait for interacting with some backing repository for retrieving, caching,
/// and modifying application data in-memory.
pub trait Repository {
    fn get_timeline(&self) -> Option<impl DerefMut<Target = Timeline<Self>> + 'static + use<Self>>;

    type EventInstanceId: Copy;

    /// Get the data of an event instance given its ID.
    fn get_event_instance(
        &self,
        id: Self::EventInstanceId,
    ) -> Result<impl DerefMut<Target = EventInstance<Self>> + 'static + use<Self>, RepoRetrievalError>;

    /// Adds a new event instance to the repository. Returns the ID of the event
    /// instance and a reference to the data.
    #[must_use]
    fn add_event_instance(
        &self,
        instance: EventInstance<Self>,
    ) -> (
        Self::EventInstanceId,
        impl DerefMut<Target = EventInstance<Self>> + 'static + use<Self>,
    );

    type EventBodyId: Copy;

    /// Get the data of an event body given its ID.
    fn get_event_body(
        &self,
        id: Self::EventBodyId,
    ) -> Result<impl DerefMut<Target = EventBody> + 'static + use<Self>, RepoRetrievalError>;

    /// Adds a new event body to the repository. Returns the ID of the event
    /// body and a reference to the data.
    #[must_use]
    fn add_event_body(
        &self,
        body: EventBody,
    ) -> (
        Self::EventBodyId,
        impl DerefMut<Target = EventBody> + 'static + use<Self>,
    );
}

#[derive(Debug)]
pub enum RepoRetrievalError {
    /// The item associated with the ID has already been retrieved. Either use
    /// the existing retrieval or release it back to the repo before retrieving
    /// it again.
    AlreadyRetrieved,
    /// The item associated with the ID could not be found.
    IdNotFound,
}
