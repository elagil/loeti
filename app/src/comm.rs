use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};

use ergot::{
    Address,
    toolkits::nusb_v0_1::{RouterStack, find_new_devices, register_router_interface},
    well_known::ErgotPingEndpoint,
};
use log::{info, warn};
use tokio::time::{Instant, interval, sleep, timeout};

use crate::{MTU, OUT_BUFFER_SIZE};

/// Check for new devices and register, if possible.
pub async fn manage_connections(stack: RouterStack) {
    let mut seen = HashSet::new();

    loop {
        let devices = find_new_devices(&HashSet::new()).await;

        for dev in devices {
            let info = dev.info.clone();
            info!("Found {:?}, registering", info);
            let _hdl = register_router_interface(&stack, dev, MTU, OUT_BUFFER_SIZE)
                .await
                .unwrap();
            seen.insert(info);
        }

        sleep(Duration::from_secs(3)).await;
    }
}

/// Ping all known devices.
pub async fn ping_all(stack: RouterStack) {
    let mut ival = interval(Duration::from_secs(1));
    let mut ctr = 0u32;
    // Attempt to remember the ping port
    let mut portmap: HashMap<u16, u8> = HashMap::new();

    loop {
        ival.tick().await;
        let nets = stack.manage_profile(|im| im.get_nets());
        info!("Nets to ping: {:?}", nets);
        for net in nets {
            let pg = ctr;
            ctr = ctr.wrapping_add(1);

            let addr = if let Some(port) = portmap.get(&net) {
                Address {
                    network_id: net,
                    node_id: 2,
                    port_id: *port,
                }
            } else {
                Address {
                    network_id: net,
                    node_id: 2,
                    port_id: 0,
                }
            };

            let start = Instant::now();
            let rr = stack
                .endpoints()
                .request_full::<ErgotPingEndpoint>(addr, &pg, None);
            let fut = timeout(Duration::from_millis(100), rr);
            let res = fut.await;
            let elapsed = start.elapsed();
            warn!("ping {}.2 w/ {}: {:?}, took: {:?}", net, pg, res, elapsed);
            if let Ok(Ok(msg)) = res {
                assert_eq!(msg.t, pg);
                portmap.insert(net, msg.hdr.src.port_id);
            } else {
                portmap.remove(&net);
            }
        }
    }
}
