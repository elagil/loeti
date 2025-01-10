//! Handles USB PD negotiation.
use defmt::{debug, error, info, Format};
use embassy_stm32::gpio::Output;
use embassy_stm32::peripherals;
use embassy_stm32::ucpd::{self, CcPhy, CcPull, CcSel, CcVState, PdPhy, Ucpd};
use embassy_time::{with_timeout, Duration};
use heapless::Vec;
use uom::si::electric_current::{self, milliampere};
use uom::si::electric_potential::{self, millivolt};
use usb_pd::header::Header;
use usb_pd::messages::pdo::PowerDataObject;
use usb_pd::messages::Message;
use usb_pd::sink::{Driver as SinkDriver, DriverState as SinkDriverState, Event, Request, Sink};
use {defmt_rtt as _, panic_probe as _};

#[derive(Debug, Format)]
enum CableOrientation {
    Normal,
    Flipped,
    DebugAccessoryMode,
}

struct UcpdSinkDriver<'d> {
    /// The UCPD PD phy instance.
    pd_phy: PdPhy<'d, peripherals::UCPD1>,

    /// Enough space for the longest non-extended data message.
    latest_message: [u8; 2048],

    /// The state of the sink driver.
    state: SinkDriverState,
}

impl<'d> UcpdSinkDriver<'d> {
    fn new(pd_phy: PdPhy<'d, peripherals::UCPD1>) -> Self {
        Self {
            pd_phy,
            latest_message: [0u8; 2048],
            state: SinkDriverState::UsbPdWait,
        }
    }
}

impl<'d> SinkDriver for UcpdSinkDriver<'d> {
    type RxError = ucpd::RxError;
    type TxError = ucpd::TxError;

    async fn init(&mut self) {}

    async fn receive_message(&mut self) -> Result<Option<Message>, Self::RxError> {
        let n = self.pd_phy.receive(self.latest_message.as_mut()).await?;

        debug!("USB PD RX: {=[u8]:?}", &self.latest_message[..n]);
        self.state = SinkDriverState::UsbPd;
        Ok(Some(Message::parse(&self.latest_message[..n])))
    }

    async fn send_message(&mut self, data: &[u8]) -> Result<(), Self::TxError> {
        debug!("USB PD TX: {=[u8]:?}", &data);
        self.pd_phy.transmit(data).await?;

        Ok(())
    }

    fn state(&mut self) -> SinkDriverState {
        self.state
    }
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
    rx_dma: peripherals::GPDMA1_CH0,
    tx_dma: peripherals::GPDMA1_CH1,
    mut ndb: Output<'static>,
) {
    ndb.set_high();

    embassy_time::Timer::after_millis(100).await;

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
    let (_cc_phy, pd_phy) = ucpd.split_pd_phy(rx_dma, tx_dma, cc_sel);

    let driver = UcpdSinkDriver::new(pd_phy);
    let mut sink = Sink::new(driver);
    sink.init().await;
    info!("Sink initialized.");

    loop {
        let result = sink.wait_for_event().await;

        match result {
            Ok(Some(event)) => {
                match event {
                    Event::SourceCapabilitiesChanged(caps) => {
                        info!("Source capabilities changed: {}", caps.pdos().len());

                        for (index, pdo) in caps.pdos().iter().enumerate() {
                            if let PowerDataObject::FixedSupply(supply) = pdo {
                                let potential_mv = supply.voltage().get::<electric_potential::millivolt>();
                                let current_ma = supply.max_current().get::<electric_current::milliampere>();

                                info!("Supply {}: {} mV, {} mA", index, potential_mv, current_ma);

                                if potential_mv == 9000 {
                                    let request = usb_pd::sink::Request::RequestPower {
                                        index,
                                        current: supply.raw_max_current(),
                                    };
                                    info!("Requesting {}", request);
                                    sink.request(request).await.unwrap();
                                }
                            }
                        }
                    }
                    Event::PowerReady => info!("Power ready."),
                    Event::ProtocolChanged => info!("Protocol changed."),
                    Event::PowerAccepted => info!("Power accepted."),
                    Event::PowerRejected => info!("Power rejected."),
                    _ => todo!(),
                };
            }
            Ok(None) => {
                info!("No event");
            }
            Err(rx_error) => info!("Receive error: {}", rx_error),
        }
    }
}
