use clap::Parser;
use clap_repl::{
    reedline::{DefaultPrompt, DefaultPromptSegment},
    ClapEditor,
};
use metime_core::{EventBody, EventInstance, MemoryRepo, Repository, TimeSpan};

mod parse;

#[derive(Parser, Debug)]
enum Command {
    Quit,
    CreateEvent {
        #[arg(default_value = "")]
        title: String,
        #[arg(short, long)]
        time_span: String,
        #[arg(long, default_value = "")]
        desc: String,
    },
    Show,
}

fn main() {
    // initialize app state
    let mut repo = MemoryRepo::new();

    // initialize REPL
    let prompt = DefaultPrompt {
        left_prompt: DefaultPromptSegment::Basic("metime".to_owned()),
        ..Default::default()
    };
    let rl = ClapEditor::<Command>::builder()
        .with_prompt(Box::new(prompt))
        .build();
    rl.repl(|command| {
        println!("{:?}", command);
        match command {
            Command::Quit => {
                println!("Goodbye!");
                std::process::exit(0);
            }
            Command::CreateEvent {
                time_span,
                title,
                desc,
            } => {
                let Some(date_time) = parse::parse_lenient_date_time(&time_span) else {
                    println!("Failed to parse date/time: {}", time_span);
                    return;
                };

                println!("Creating event at: {}", date_time.format("%c"));

                let event_body = EventBody {
                    summary: title,
                    description: desc,
                };
                let (body_id, _) = repo.add_event_body(event_body);

                let event_instance = EventInstance {
                    time_span: TimeSpan::Instant(date_time),
                    body: body_id,
                };
                let (_, _) = repo.add_event_instance(event_instance);
            }
            Command::Show => {
                println!("{:#?}", &repo);
            }
        }
    })
}
