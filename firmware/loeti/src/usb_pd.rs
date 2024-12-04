//! Handles USB PD negotiation.
use defmt::{error, info, Format};
use embassy_stm32::peripherals;
use embassy_stm32::ucpd::{self, CcPhy, CcPull, CcSel, CcVState, Ucpd};
use embassy_time::{with_timeout, Duration};
use {defmt_rtt as _, panic_probe as _};

#[derive(Debug, Format)]
enum CableOrientation {
    Normal,
    Flipped,
    DebugAccessoryMode,
}

// Returns true when the cable
async fn wait_attached<T: ucpd::Instance>(cc_phy: &mut CcPhy<'_, T>) -> CableOrientation {
    loop {
        let (cc1, cc2) = cc_phy.vstate();
        if cc1 == CcVState::LOWEST && cc2 == CcVState::LOWEST {
            // Detached, wait until attached by monitoring the CC lines.
            cc_phy.wait_for_vstate_change().await;
            continue;
        }

        // Attached, wait for CC lines to be stable for tCCDebounce (100..200ms).
        if with_timeout(Duration::from_millis(100), cc_phy.wait_for_vstate_change())
            .await
            .is_ok()
        {
            // State has changed, restart detection procedure.
            continue;
        };

        // State was stable for the complete debounce period, check orientation.
        return match (cc1, cc2) {
            (_, CcVState::LOWEST) => CableOrientation::Normal,  // CC1 connected
            (CcVState::LOWEST, _) => CableOrientation::Flipped, // CC2 connected
            _ => CableOrientation::DebugAccessoryMode,          // Both connected (special cable)
        };
    }
}

/// Handle USB PD negotiation.
#[embassy_executor::task]
pub async fn ucpd_task(
    mut ucpd: Ucpd<'static, peripherals::UCPD1>,
    rx_dma: peripherals::DMA1_CH1,
    tx_dma: peripherals::DMA1_CH2,
) {
    ucpd.cc_phy().set_pull(CcPull::Sink);

    info!("Waiting for USB connection...");
    let cable_orientation = wait_attached(ucpd.cc_phy()).await;
    info!("USB cable connected, orientation: {}", cable_orientation);

    let cc_sel = match cable_orientation {
        CableOrientation::Normal => {
            info!("Starting PD communication on CC1 pin");
            CcSel::CC1
        }
        CableOrientation::Flipped => {
            info!("Starting PD communication on CC2 pin");
            CcSel::CC2
        }
        CableOrientation::DebugAccessoryMode => panic!("No PD communication in DAM"),
    };
    let (_cc_phy, mut pd_phy) = ucpd.split_pd_phy(rx_dma, tx_dma, cc_sel);

    loop {
        // Enough space for the longest non-extended data message.
        let mut buf = [0_u8; 30];
        match pd_phy.receive(buf.as_mut()).await {
            Ok(n) => info!("USB PD RX: {=[u8]:?}", &buf[..n]),
            Err(e) => error!("USB PD RX: {}", e),
        }
    }
}
