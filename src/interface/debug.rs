use std::time::Duration;
use tokio::time::sleep;

use crate::{
    core::{
        peering::PeerQuery,
        routing::RouteQuery,
        state::Singleton,
        ticket::{TicketMessage, TicketQuery},
        tracing::Trace,
    },
    private::http::client::ticketing::request_ticket_synchronization,
};

pub async fn ticket_query(
    destination_id: Option<String>,
    singleton: Singleton,
) -> anyhow::Result<TicketQuery> {
    let query = TicketQuery { tickets: None };

    let url = format!(
        "http://{}:{}{}",
        singleton.configuration.private_http.host,
        singleton.configuration.private_http.port,
        singleton.configuration.private_http.ticket_sync_endpoint
    );

    if let TicketMessage::TicketQuery(query) = request_ticket_synchronization(
        TicketMessage::TicketQuery(query),
        None,
        destination_id,
        url.clone(),
    )
    .await?
    {
        if let Some(tickets) = &query.tickets {
            for ticket_state in tickets.iter() {
                log::info!("{ticket_state}");
            }
        }

        Ok(query)
    } else {
        Err(anyhow::Error::msg(
            "unexpected ticket variant received during synchronization",
        ))
    }
}

pub async fn peer_query(
    destination_id: Option<String>,
    singleton: Singleton,
) -> anyhow::Result<PeerQuery> {
    let query = PeerQuery { peers: None };

    let url = format!(
        "http://{}:{}{}",
        singleton.configuration.private_http.host,
        singleton.configuration.private_http.port,
        singleton.configuration.private_http.ticket_sync_endpoint
    );

    if let TicketMessage::PeerQuery(query) = request_ticket_synchronization(
        TicketMessage::PeerQuery(query),
        None,
        destination_id,
        url.clone(),
    )
    .await?
    {
        if let Some(peers) = &query.peers {
            for peer in peers {
                log::info!("{peer}");
            }
        }

        Ok(query)
    } else {
        Err(anyhow::Error::msg(
            "unexpected ticket variant received during synchronization",
        ))
    }
}

pub async fn route_query(
    destination_id: Option<String>,
    singleton: Singleton,
) -> anyhow::Result<RouteQuery> {
    let query = RouteQuery {
        routes: None,
        destination_ids: None,
        gateway_ids: None,
    };

    let url = format!(
        "http://{}:{}{}",
        singleton.configuration.private_http.host,
        singleton.configuration.private_http.port,
        singleton.configuration.private_http.ticket_sync_endpoint
    );

    if let TicketMessage::RouteQuery(query) = request_ticket_synchronization(
        TicketMessage::RouteQuery(query),
        None,
        destination_id,
        url.clone(),
    )
    .await?
    {
        if let Some(routes) = &query.routes {
            for route in routes {
                log::info!("{route}");
            }
        }

        Ok(query)
    } else {
        Err(anyhow::Error::msg(
            "unexpected ticket variant received during synchronization",
        ))
    }
}

pub async fn trace(destination_id: String, singleton: Singleton) -> anyhow::Result<Trace> {
    let mut trace = Trace {
        events: vec![],
        destination_id,
        request_id: None,
        start_time: None,
        stop_time: None,
    };

    let url = format!(
        "http://{}:{}{}",
        singleton.configuration.private_http.host,
        singleton.configuration.private_http.port,
        singleton.configuration.private_http.ticket_sync_endpoint
    );

    if let TicketMessage::BeginTrace(initial_trace) =
        request_ticket_synchronization(TicketMessage::BeginTrace(trace), None, None, url.clone())
            .await?
    {
        trace = initial_trace;
        let mut hop_count = 0;

        loop {
            if let TicketMessage::PollTrace(polled_trace) = request_ticket_synchronization(
                TicketMessage::PollTrace(trace.clone()),
                None,
                None,
                url.clone(),
            )
            .await?
            {
                if polled_trace != trace {
                    trace = polled_trace;
                    for i in hop_count..trace.events.len() {
                        log::info!("{}", trace.events[i])
                    }
                    hop_count = trace.events.len();
                } else if polled_trace.stop_time.is_some() {
                    break;
                } else {
                    sleep(Duration::from_millis(100)).await;
                }
            } else {
                return Err(anyhow::Error::msg(
                    "unexpected ticket variant received during synchronization",
                ));
            }
        }

        request_ticket_synchronization(
            TicketMessage::DrainTrace(trace.clone()),
            None,
            None,
            url.clone(),
        )
        .await?;
    } else {
        return Err(anyhow::Error::msg(
            "unexpected ticket variant received during synchronization",
        ));
    }

    Ok(trace)
}
