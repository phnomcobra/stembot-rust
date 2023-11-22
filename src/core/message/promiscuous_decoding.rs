use crate::core::{
    backlog::push_message_collection_to_backlog,
    message::{Message, MessageCollection},
    state::Singleton,
};

pub async fn decode_message_collection(
    singleton: Singleton,
    inbound_message_collection: &mut MessageCollection,
) {
    let origin_id = inbound_message_collection.origin_id.clone();
    let destination_id = inbound_message_collection.destination_id.clone();
    for message in &mut inbound_message_collection.messages {
        decode_message(singleton.clone(), message, &origin_id, &destination_id).await;
    }
}

async fn decode_message(
    singleton: Singleton,
    message: &mut Message,
    origin_id: &str,
    destination_id: &Option<String>,
) {
    match message {
        Message::TraceRequest(trace_request) => {
            let trace_event = trace_request.process(singleton.clone());

            let message_collection = MessageCollection {
                messages: vec![Message::TraceEvent(trace_event)],
                destination_id: Some(origin_id.to_owned()),
                origin_id: singleton.configuration.id.clone(),
            };

            push_message_collection_to_backlog(message_collection, singleton.clone()).await
        }
        Message::TraceResponse(trace_response) => {
            let trace_event = trace_response.process(singleton.clone());

            if destination_id.is_some() {
                let message_collection = MessageCollection {
                    messages: vec![Message::TraceEvent(trace_event)],
                    destination_id: destination_id.clone(),
                    origin_id: singleton.configuration.id.clone(),
                };

                push_message_collection_to_backlog(message_collection, singleton.clone()).await
            }
        }
        _ => {}
    }
}
