use clap::Parser;
use clap_repl::{
    reedline::{DefaultPrompt, DefaultPromptSegment},
    ClapEditor,
};
use metime_core::MemoryRepo;

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
                let Some(time_span) = parse::parse_lenient_time_span(&time_span) else {
                    println!("Failed to parse date/time: {}", time_span);
                    return;
                };

                println!("Creating event at: {}", time_span);

                let _ = metime_core::add_event(&mut repo, time_span, title, desc);
            }
            Command::Show => {
                println!("{:#?}", &repo);
            }
        }
    })
}
