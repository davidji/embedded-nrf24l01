
use core::fmt;

use crate::device::{ Device, UsingDevice };
use crate::command::{ FlushTx, ReadRxPayload, ReadRxPayloadWidth, WriteTxPayload };
use crate::rxtx::{ Received, SendReceiveResult };
use crate::registers::{ FifoStatus, Status };
use crate::config::Configuration;

/// In PTX mode, the device transmits packets immediately, and receives packets
/// only as acknowledge payloads. It's the complement to PRX mode
pub struct PtxMode<D: Device> {
    device: D,
}

impl<D: Device> fmt::Debug for PtxMode<D> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "TxMode")
    }
}

impl <D: Device> UsingDevice<D> for PtxMode<D> {
    fn device(&mut self) -> &mut D {
        &mut self.device
    }
}

impl <D: Device> Configuration<D> for PtxMode<D> {

}

impl<D: Device> PtxMode<D> {

    /// Send asynchronously
    pub fn send_receive(
            &mut self,
            send: Option<&[u8]>
        ) -> Result<SendReceiveResult, D::Error> {
        let (status, fifo_status) = self.device.read_register::<FifoStatus>()?;
        let dropped = match status.max_rt() {
            true => {
                self.device.send_command(&FlushTx)?;
                let mut clear_max_rt = Status(0);
                clear_max_rt.set_max_rt(true);
                self.device.write_register(clear_max_rt)?;
                true
            },
            false => false
        };

        let sent = match (fifo_status.tx_full(), send) {
                (true, _) => false,
                (false, None) => false,
                (false, Some(payload)) => {
                    self.device.send_command(&WriteTxPayload::new(payload))?;
                    self.device.ce_enable();
                    true
                }
            };

        let received = match fifo_status.rx_empty() {
                true => None,
                false => {
                    let (_, payload_width) = self.device.send_command(&ReadRxPayloadWidth)?;
                    let (_, payload) = self.device.send_command(&ReadRxPayload::new(payload_width as usize))?;
                    Some(Received { pipe: status.rx_p_no(), payload: payload})
                }
            };
        Ok(SendReceiveResult { sent, received, dropped })
    }
}

impl<D: Device> PtxMode<D> {
    /// Relies on everything being set up by `StandbyMode::ptx()`, from which it is called
    pub(crate) fn new(device: D) -> Self {
        PtxMode { device }
    }
}
