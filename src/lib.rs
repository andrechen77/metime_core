#![feature(precise_capturing_in_traits)]

use std::ops::DerefMut;

mod domain;
mod repository;

pub use domain::{EventBody, EventInstance, TimeSpan};
pub use repository::{memory_repo::MemoryRepo, Repository};

pub fn add_event<R: Repository>(
    repo: &mut R,
    time_span: TimeSpan,
    title: String,
    desc: String,
) -> (
    R::EventInstanceId,
    R::EventBodyId,
    impl DerefMut<Target = EventInstance<R>> + 'static,
    impl DerefMut<Target = EventBody> + 'static,
) {
    let event_body = EventBody {
        summary: title,
        description: desc,
    };
    let (body_id, body) = repo.add_event_body(event_body);

    let time = time_span.earliest();

    let event_instance = EventInstance {
        time_span,
        body: body_id,
    };
    let (instance_id, instance) = repo.add_event_instance(event_instance);

    let mut timeline = repo.get_timeline().unwrap();
    timeline.events.insert(time, instance_id);

    (instance_id, body_id, instance, body)
}
