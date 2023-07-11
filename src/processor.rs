use crate::{
    config::Configuration,
    message::{Message, MessageCollection},
};

pub fn process_message_collection<T: Into<MessageCollection>, U: Into<Configuration>>(
    inbound_message_collection: T,
    configuration: U,
) -> MessageCollection {
    let configuration = configuration.into();

    let inbound_message_collection = inbound_message_collection.into();

    let mut outbound_message_collection = MessageCollection {
        messages: vec![],
        origin_id: configuration.id.clone(),
    };

    for message in inbound_message_collection.messages {
        match message {
            Message::Ping => outbound_message_collection.messages.push(Message::Pong),
            _ => log::warn!("processor not implemented for {:?}", &message),
        }
    }

    outbound_message_collection
}
