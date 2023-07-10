use anyhow::Result;
use hello_rust_backend::etcd::EtcdClients;
use std::time::Duration;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::watch;

// tracing
use hello_rust_backend::tracing_utils::set_up_logging;
use tracing::{event, span, Instrument, Level};

use hello_rust_backend::cluster_management::{
    create_a_sync_lock_record, get_all_worker_records, get_current_cluster_members_count,
};
use hello_rust_backend::do_some_stuff_with_etcd;

#[tokio::main]
async fn main() -> Result<()> {
    set_up_logging()?;

    // let (send, mut recv): (Sender<()>, _) = channel(1);
    let (tx, rx) = watch::channel(());

    // Env vars! -----------------------------------
    let mut retry_wait_seconds = 1;
    let settings_map = loop {
        let settings_map = hello_rust_backend::settings::get_settings();
        match settings_map {
            Err(error) => {
                event!(Level::ERROR, "Error obtaining settings: {}", error);
                tokio::time::sleep(Duration::from_secs(retry_wait_seconds)).await;
                if retry_wait_seconds < 300 {
                    retry_wait_seconds += retry_wait_seconds
                };
            }
            Ok(settings_map) => break settings_map,
        }
    };
    event!(Level::INFO, "Settings successfully obtained.");
    event!(Level::INFO, "{:#?}", settings_map);

    dbg!(std::env::var("NO_OTLP")
        .unwrap_or_else(|_| "0".to_owned())
        .as_str());

    let span = span!(Level::TRACE, "talk to etcd");

    let node_name = std::env::var("HOSTNAME")?;

    let etcd_clients = async {
        // This is correct! If we yield here, the span will be exited,
        // and re-entered when we resume.
        if settings_map.etcd_url.is_some() {
            event!(Level::INFO, "About to try talking to etcd!");

            event!(Level::INFO, "Clustered setting: {}", settings_map.clustered);

            let result = do_some_stuff_with_etcd(
                &settings_map.etcd_url.expect("should be valid string"),
                node_name.as_str(),
            )
            .await;

            match result {
                Ok(ref result) => {
                    dbg!("{:#?}", result);
                    event!(Level::INFO, "{:#?}", result);
                }
                Err(ref error) => event!(Level::ERROR, "Error while talking to etcd. {:#?}", error),
            }
            event!(Level::INFO, "Finished initial talking to etcd.");
            result.ok()
        } else {
            event!(Level::WARN, "No etcd endpoint set.");
            None
        }
    }
    // instrument the async block with the span...
    .instrument(span)
    // ...and await it.
    .await;

    async fn some_operation(message: &str, duration: Duration, receiver: watch::Receiver<()>) {
        loop {
            tokio::time::sleep(duration).await;

            let span = span!(Level::TRACE, "message span");
            let _enter = span.enter();
            event!(Level::INFO, message);

            if receiver.has_changed().unwrap_or(true) {
                break;
            };
        }
        event!(Level::INFO, "Task shutting down. ({})", message);

        // sender goes out of scope ...
    }

    let _op1 = tokio::spawn(some_operation(
        "Hello World!",
        Duration::from_secs(40),
        rx.clone(),
    ));

    let _op2 = tokio::spawn(some_operation(
        "hello world from a shorter loop!",
        Duration::from_secs(30),
        rx.clone(),
    ));

    async fn loop_getting_cluster_members(mut etcd_clients: EtcdClients, node_name: &str) {
        loop {
            async {
                let list = get_all_worker_records(&mut etcd_clients.kv).await;
                if let Ok(list) = list {
                    let mapped_kv: Vec<_> = list
                        .kvs
                        .iter()
                        .map(|element| {
                            std::str::from_utf8(&element.key)
                                .expect("Should be valid utf8")
                                .strip_prefix("/nodes/")
                                .expect("should be formatted with /nodes/ at start")
                        })
                        .collect();

                    let current_worker_index = mapped_kv.iter().position(|x| *x == node_name);
                    let workers_count = list.count;

                    // This should be equal to the total number of sync partitions in DynamoDB.
                    // Perhaps there should be a way to calculate this automatically?! For now it
                    // is fine as a compile time constant.
                    let total_number_of_sync_partitions = 21;

                    let sync_records_to_claim: Option<Vec<usize>> =
                        current_worker_index.map(|current_worker_index| {
                            ((current_worker_index)..total_number_of_sync_partitions)
                                .step_by(workers_count as usize)
                                .collect()
                        });

                    dbg!(&sync_records_to_claim);

                    let n_sync_records_to_claim = sync_records_to_claim.as_ref().map(|x| x.len());

                    // TODO: THIS NEEDS THE LEASE SOMEHOW. MAYBE THERE SHOULD BE A CHANNEL FROM
                    // THE CLUSTER MANAGEMENT THREAD TO THE WORKER STUFF, WOULD BOTH TELL THE
                    // WORKER BIT WHEN TO REFRESH (E.G. HAD TO GET A NEW LEASE) AND PROVIDE THE
                    // UP-TO-DATE LEASE_ID. OR MAYBE THIS SHOULD ALL BE IN THAT THREAD AS WELL
                    // ANYWAY?!?!??!
                    let result = if let Some(sync_records_to_claim) = sync_records_to_claim {
                        for i in sync_records_to_claim {
                            create_a_sync_lock_record(
                                &mut etcd_clients.kv,
                                1234,
                                node_name.to_owned(),
                                &i.to_string(),
                            )
                            .await;
                        }
                    };

                    dbg!(result);

                    event!(
                        Level::DEBUG,
                        workers_count,
                        current_worker = node_name,
                        current_worker_index,
                        n_sync_records_to_claim,
                        "kvs strings: {:#?}",
                        mapped_kv
                    );
                }
            }
            .instrument(span!(Level::INFO, "get cluster members list and count"))
            .await;

            tokio::time::sleep(Duration::from_secs(20)).await;
        }
    }

    if let Some(etcd_clients) = etcd_clients {
        let mut rx2 = rx.clone();
        tokio::spawn(async move {
            tokio::select! {
                _ = loop_getting_cluster_members(etcd_clients, &node_name) => {},
                _ = rx2.changed() => {
                    dbg!("rx shutdown channel changed");
                 }
            }
        });
    }

    let mut sigterm_stream = signal(SignalKind::terminate())?;
    let mut sigint_stream = signal(SignalKind::interrupt())?;
    tokio::select! {
        _ = sigterm_stream.recv() => {}
        _ = sigint_stream.recv() => {}
    }

    let span = span!(Level::TRACE, "Shutting down tasks");
    async {
        // send shutdown signal to application and wait
        tx.send(())?;

        // // Wait for the tasks to finish.
        // //
        // // We drop our sender first because the recv() call otherwise
        // // sleeps forever.
        // drop(send);
        drop(rx);
        tx.closed().await;

        event!(Level::TRACE, "All tasks shutdown.");

        // When every sender has gone out of scope, the recv call
        // will return with an error. We ignore the error.
        // let _ = recv.recv().await;

        Ok::<(), anyhow::Error>(())
    }
    // instrument the async block with the span...
    .instrument(span)
    // ...and await it.
    .await?;

    // Shutdown trace pipeline
    opentelemetry::global::shutdown_tracer_provider();

    println!("Shutdown complete!");

    Ok(())
}
