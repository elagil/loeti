//! Handles USB PD negotiation.
use crate::MAX_SUPPLY_CURRENT_MA_SIG;
use assign_resources::assign_resources;
use defmt::{info, warn, Format};
use embassy_futures::select::{select, Either};
use embassy_stm32::gpio::Output;
use embassy_stm32::ucpd::{self, CcPhy, CcPull, CcSel, CcVState, PdPhy, Ucpd};
use embassy_stm32::{bind_interrupts, peripherals, Peri};
use embassy_time::{with_timeout, Duration, Timer};
use uom::si::electric_current;
use usbpd::protocol_layer::message::request;
use usbpd::sink::device_policy_manager::DevicePolicyManager;
use usbpd::sink::policy_engine::Sink;
use usbpd::timers::Timer as SinkTimer;
use usbpd::Driver as SinkDriver;

use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    UCPD1 => ucpd::InterruptHandler<peripherals::UCPD1>;
});

assign_resources! {
    #[allow(missing_docs)]
    ucpd: UcpdResources {
        ucpd: UCPD1,
        pin_cc1: PB6,
        pin_cc2: PB4,
        rx_dma: DMA1_CH1,
        tx_dma: DMA1_CH2,
    }
}

#[derive(Debug, Format)]
enum CableOrientation {
    Normal,
    Flipped,
    DebugAccessoryMode,
}

struct UcpdSinkDriver<'d> {
    /// The UCPD PD phy instance.
    pd_phy: PdPhy<'d, peripherals::UCPD1>,
}

impl<'d> UcpdSinkDriver<'d> {
    fn new(pd_phy: PdPhy<'d, peripherals::UCPD1>) -> Self {
        Self { pd_phy }
    }
}

impl SinkDriver for UcpdSinkDriver<'_> {
    async fn wait_for_vbus(&self) {
        // The sink policy engine is only running when attached. Therefore VBus is present.
    }

    async fn receive(&mut self, buffer: &mut [u8]) -> Result<usize, usbpd::DriverRxError> {
        self.pd_phy.receive(buffer).await.map_err(|err| match err {
            ucpd::RxError::Crc | ucpd::RxError::Overrun => usbpd::DriverRxError::Discarded,
            ucpd::RxError::HardReset => usbpd::DriverRxError::HardReset,
        })
    }

    async fn transmit(&mut self, data: &[u8]) -> Result<(), usbpd::DriverTxError> {
        self.pd_phy.transmit(data).await.map_err(|err| match err {
            ucpd::TxError::Discarded => usbpd::DriverTxError::Discarded,
            ucpd::TxError::HardReset => usbpd::DriverTxError::HardReset,
        })
    }

    async fn transmit_hard_reset(&mut self) -> Result<(), usbpd::DriverTxError> {
        self.pd_phy.transmit_hardreset().await.map_err(|err| match err {
            ucpd::TxError::Discarded => usbpd::DriverTxError::Discarded,
            ucpd::TxError::HardReset => usbpd::DriverTxError::HardReset,
        })
    }
}

async fn wait_detached<T: ucpd::Instance>(cc_phy: &mut CcPhy<'_, T>) {
    loop {
        let (cc1, cc2) = cc_phy.vstate();
        if cc1 == CcVState::LOWEST && cc2 == CcVState::LOWEST {
            return;
        }
        cc_phy.wait_for_vstate_change().await;
    }
}

// Returns true when the cable was attached.
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

struct EmbassySinkTimer {}

impl SinkTimer for EmbassySinkTimer {
    async fn after_millis(milliseconds: u64) {
        Timer::after_millis(milliseconds).await
    }
}

struct Device {}

impl DevicePolicyManager for Device {
    /// Notify the device that it shall transition to a new power level.
    ///
    /// The device is informed about the request that was accepted by the source.
    async fn transition_power(&mut self, accepted: &request::PowerSource) {
        match accepted {
            request::PowerSource::FixedVariableSupply(supply) => MAX_SUPPLY_CURRENT_MA_SIG
                .signal(supply.max_operating_current().get::<electric_current::milliampere>() as f32),
            _ => (),
        }
    }
}

/// Handle USB PD negotiation.
#[embassy_executor::task]
pub async fn ucpd_task(mut ucpd_resources: UcpdResources, mut ndb_pin: Output<'static>) {
    loop {
        let mut ucpd = Ucpd::new(
            ucpd_resources.ucpd.reborrow(),
            Irqs {},
            ucpd_resources.pin_cc1.reborrow(),
            ucpd_resources.pin_cc2.reborrow(),
            Default::default(),
        );

        ucpd.cc_phy().set_pull(CcPull::Sink);
        ndb_pin.set_high();

        info!("Waiting for USB connection");
        let cable_orientation = wait_attached(ucpd.cc_phy()).await;
        info!("USB cable attached, orientation: {}", cable_orientation);

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
        let (mut cc_phy, pd_phy) = ucpd.split_pd_phy(
            ucpd_resources.rx_dma.reborrow(),
            ucpd_resources.tx_dma.reborrow(),
            cc_sel,
        );

        let driver = UcpdSinkDriver::new(pd_phy);
        let mut sink: Sink<UcpdSinkDriver<'_>, EmbassySinkTimer, _> = Sink::new(driver, Device {});
        info!("Run sink");

        match select(sink.run(), wait_detached(&mut cc_phy)).await {
            Either::First(result) => warn!("Sink loop broken with result: {}", result),
            Either::Second(_) => {
                info!("Detached");
                continue;
            }
        }
    }
}
