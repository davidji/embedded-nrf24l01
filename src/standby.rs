use core::fmt;

use crate::config::{ Configuration, auto_retransmit, auto_ack };
use crate::device::{ Device, UsingDevice };
use crate::rx::RxMode;
use crate::tx::TxMode;
use crate::ptx::PtxMode;
use crate::registers::{ Feature, Dynpd };
use crate::PIPES_COUNT;

/// Represents **Standby-I** mode
///
/// This represents the state the device is in inbetween TX or RX
/// mode.
pub struct StandbyMode<D: Device> {
    device: D,
}

impl<D: Device> fmt::Debug for StandbyMode<D> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "StandbyMode")
    }
}

impl<D: Device> UsingDevice<D> for StandbyMode<D> {
    fn device(&mut self) -> &mut D {
        &mut self.device
    }
}

impl<D: Device> Configuration<D> for StandbyMode<D> {

}


impl<D: Device> StandbyMode<D> {
    pub fn power_up(mut device: D) -> Result<Self, (D, D::Error)> {
        match device.update_config(|config| config.set_pwr_up(true)) {
            Ok(()) => Ok(StandbyMode { device }),
            Err(e) => Err((device, e)),
        }
    }

    pub(crate) fn from_rx_tx(mut device: D) -> Self {
        device.ce_disable();
        StandbyMode { device }
    }

    /// Go into RX mode
    pub fn rx(self) -> Result<RxMode<D>, (D, D::Error)> {
        let mut device = self.device;

        match device.update_config(|config| config.set_prim_rx(true)) {
            Ok(()) => {
                device.ce_enable();
                Ok(RxMode::new(device))
            }
            Err(e) => Err((device, e)),
        }
    }

    /// Go into TX mode
    pub fn tx(self) -> Result<TxMode<D>, (D, D::Error)> {
        let mut device = self.device;
        
        match device.update_config(|config| config.set_prim_rx(false)) {
            Ok(()) => {
                // No need to device.ce_enable(); yet
                Ok(TxMode::new(device))
            }
            Err(e) => Err((device, e)),
        }
    }

    pub fn ptx(self, delay: u8, retries: u8) -> Result<PtxMode<D>, (D, D::Error)> {

        let mut device = self.device;
        let mut config_ptx =  || {
            device.write_register(auto_ack(&[ true; 6 ]))?;
            device.write_register(auto_retransmit(delay, retries))?;
            // Enable ack payload and dynamic payload features
            device.update_register::<Feature, _, _>(|feature| {
                feature.set_en_ack_pay(true);
                feature.set_en_dpl(true);
            })?;
            // Enable dynamic payload on all pipes
            device.write_register(Dynpd::from_bools(&[true; PIPES_COUNT]))?;
            Ok(())
        };

        match config_ptx() {
            Ok(()) => {
                match device.update_config(|config| config.set_prim_rx(false)) {
                    Ok(()) => {
                        // No need to device.ce_enable(); yet
                        Ok(PtxMode::new(device))
                    }
                    Err(e) => Err((device, e)),
                }
            },
            Err(e) => Err((device, e))
        }
    }
}

