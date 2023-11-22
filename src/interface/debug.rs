use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::sleep;

use crate::{
    core::{
        peering::PeerQuery,
        routing::RouteQuery,
        state::Singleton,
        ticket::{Ticket, TicketQuery},
        tracing::Trace,
    },
    private::http::client::ticketing::request_ticket_synchronization,
};

fn now() -> anyhow::Result<u128> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())
}

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

    if let Ticket::TicketQuery(query) = request_ticket_synchronization(
        Ticket::TicketQuery(query),
        None,
        destination_id,
        url.clone(),
    )
    .await?
    {
        if let Some(tickets) = &query.tickets {
            for ticket in tickets.iter() {
                let completed = if ticket.1 .1.is_some() {
                    "(completed)"
                } else {
                    ""
                };
                log::info!("{} {completed}", ticket.1 .0);
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

    if let Ticket::PeerQuery(query) =
        request_ticket_synchronization(Ticket::PeerQuery(query), None, destination_id, url.clone())
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

    if let Ticket::RouteQuery(query) =
        request_ticket_synchronization(Ticket::RouteQuery(query), None, destination_id, url.clone())
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
        period: None,
        destination_id,
        request_id: None,
    };

    let url = format!(
        "http://{}:{}{}",
        singleton.configuration.private_http.host,
        singleton.configuration.private_http.port,
        singleton.configuration.private_http.ticket_sync_endpoint
    );

    if let Ticket::BeginTrace(initial_trace) =
        request_ticket_synchronization(Ticket::BeginTrace(trace), None, None, url.clone()).await?
    {
        trace = initial_trace;
        let mut diff_time = now()?;
        let mut hop_count = 0;

        loop {
            if let Ticket::PollTrace(polled_trace) = request_ticket_synchronization(
                Ticket::PollTrace(trace.clone()),
                None,
                None,
                url.clone(),
            )
            .await?
            {
                if polled_trace != trace {
                    trace = polled_trace;
                    diff_time = now()?;
                    for i in hop_count..trace.events.len() {
                        log::info!("{}", trace.events[i])
                    }
                    hop_count = trace.events.len();
                } else if now()? - diff_time > singleton.configuration.ticketexpiration.into() {
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

        request_ticket_synchronization(Ticket::DrainTrace(trace.clone()), None, None, url.clone())
            .await?;
    } else {
        return Err(anyhow::Error::msg(
            "unexpected ticket variant received during synchronization",
        ));
    }

    Ok(trace)
}
