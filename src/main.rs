use crate::dod::Message;
use dem::open_demo;
use dem::types::{FrameData, MessageData, NetMessage};
use std::convert::identity;
use std::env::args;

mod dod;

fn main() {
    let args = args().collect::<Vec<_>>();
    let demo_path = args.get(1).unwrap();

    let demo = open_demo(demo_path).unwrap();

    demo.directory
        .entries
        .iter()
        .flat_map(|entry| &entry.frames)
        .filter_map(|frame| match &frame.frame_data {
            FrameData::NetworkMessage(frame_data) => Some(frame_data),
            _ => None,
        })
        .filter_map(|frame_data| {
            if let MessageData::Parsed(messages) = &frame_data.1.messages {
                Some(messages)
            } else {
                None
            }
        })
        .flat_map(identity)
        .filter_map(|net_msg| match net_msg {
            NetMessage::UserMessage(user_message) => Some(user_message),
            NetMessage::EngineMessage(_) => None,
        })
        .filter_map(|user_msg| {
            let dod_msg = Message::try_from(user_msg).ok()?;

            Some(dod_msg)
        })
        .filter(|dod_msg| {
            matches!(
                dod_msg,
                Message::CancelProg(_)
                    | Message::ClanTimer(_)
                    | Message::DeathMsg(_)
                    | Message::ObjScore(_)
                    | Message::PClass(_)
                    | Message::PTeam(_)
                    | Message::PlayersIn(_)
                    | Message::RoundState(_)
                    | Message::ScoreShort(_)
                    | Message::StartProg(_)
                    | Message::TeamScore(_)
            )
        })
        .for_each(|dod_msg| {
            println!("{:?}", dod_msg);
        });

    // for entry in &demo.directory.entries {
    //     for frame in &entry.frames {
    //         match &frame.frame_data {
    //             FrameData::NetworkMessage(frame_data) => {
    //                 if let MessageData::Parsed(messages) = &frame_data.1.messages {
    //                     for message in messages {
    //                         if let NetMessage::UserMessage(user_msg) = message {
    //                             if let Ok(dod_msg) = crate::dod::Message::try_from(user_msg) {
    //                                 println!("{:?}", dod_msg)
    //                             } else {
    //                                 println!(
    //                                     "{:?} {:?}",
    //                                     unsafe {
    //                                         String::from_utf8_unchecked(user_msg.name.clone())
    //                                     }
    //                                     .trim_end_matches('\x00'),
    //                                     user_msg.data
    //                                 );
    //                             }
    //                         }
    //                     }
    //                 }
    //             }
    //             _ => {}
    //         }
    //     }
    // }
}
