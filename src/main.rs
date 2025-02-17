use dem::open_demo;
use dem::types::{FrameData, MessageData, NetMessage};
use std::env::args;

mod dod;

fn main() {
    let args = args().collect::<Vec<_>>();
    let demo_path = args.get(1).unwrap();

    let demo = open_demo(demo_path).unwrap();

    for entry in &demo.directory.entries {
        for frame in &entry.frames {
            match &frame.frame_data {
                FrameData::NetworkMessage(frame_data) => {
                    if let MessageData::Parsed(messages) = &frame_data.1.messages {
                        for message in messages {
                            if let NetMessage::UserMessage(user_msg) = message {
                                if let Ok(dod_msg) = crate::dod::Message::try_from(user_msg) {
                                    println!("{:?}", dod_msg)
                                } else {
                                    println!(
                                        "{:?} {:?}",
                                        unsafe {
                                            String::from_utf8_unchecked(user_msg.name.clone())
                                        }
                                        .trim_end_matches('\x00'),
                                        user_msg.data
                                    );
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
}
