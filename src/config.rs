use crate::command::{FlushRx, FlushTx, Nop};
use crate::device::{ Device, UsingDevice };
use crate::registers::{
    Config, Dynpd, EnAa, EnRxaddr, Feature, RfCh, RfSetup, SetupAw, SetupRetr, Status, TxAddr,
};
use crate::PIPES_COUNT;

/// Supported air data rates.
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum DataRate {
    /// 250 Kbps
    R250Kbps,
    /// 1 Mbps
    R1Mbps,
    /// 2 Mbps
    R2Mbps,
}

impl Default for DataRate {
    fn default() -> DataRate {
        DataRate::R1Mbps
    }
}

/// Supported CRC modes
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum CrcMode {
    /// Disable all CRC generation/checking
    Disabled,
    /// One byte checksum
    OneByte,
    /// Two bytes checksum
    TwoBytes,
}

impl CrcMode {
    fn set_config(&self, config: &mut Config) {
        let (en_crc, crco) = match *self {
            CrcMode::Disabled => (false, false),
            CrcMode::OneByte => (true, false),
            CrcMode::TwoBytes => (true, true),
        };
        config.set_en_crc(en_crc);
        config.set_crco(crco);
    }
}

pub fn auto_retransmit( delay: u8, count: u8) -> SetupRetr {
    let mut register = SetupRetr(0);
    register.set_ard(delay);
    register.set_arc(count);
    register
}

pub fn auto_ack(bools: &[bool; PIPES_COUNT]) -> EnAa {
    // Convert back
    EnAa::from_bools(bools)
}

pub struct Interrupts {
    // Data has been received
    pub rx_dr: bool,
    // Data has been acknowledged, and so there's a newly free slot in the TX FIFO
    pub tx_ds: bool,
    // The maximum retries has been reached for the packet at the head of the TX fifo,
    // so transimission has stopped.
    pub max_rt: bool,
}

/// Configuration methods
///
/// These seem to work in all modes
pub trait Configuration<D> : UsingDevice<D> 
where D: Device {

    /// Flush the RX queue, discarding any unread packets
    fn flush_rx(&mut self) -> Result<(), D::Error> {
        self.device().send_command(&FlushRx)?;
        Ok(())
    }

    /// Flush TX queue, discarding any unsent packets
    fn flush_tx(&mut self) -> Result<(), D::Error> {
        self.device().send_command(&FlushTx)?;
        Ok(())
    }

    /// Get frequency offset (channel)
    fn get_frequency(&mut self) -> Result<u8, D::Error> {
        let (_, register) = self.device().read_register::<RfCh>()?;
        let freq_offset = register.rf_ch();
        Ok(freq_offset)
    }

    /// Set frequency offset (channel)
    fn set_frequency(
        &mut self,
        freq_offset: u8,
    ) -> Result<(), D::Error> {
        assert!(freq_offset < 126);

        let mut register = RfCh(0);
        register.set_rf_ch(freq_offset);
        self.device().write_register(register)?;

        Ok(())
    }

    /// power: `0`: -18 dBm, `3`: 0 dBm
    fn set_rf(
        &mut self,
        rate: &DataRate,
        power: u8,
    ) -> Result<(), D::Error> {
        assert!(power < 0b100);
        let mut register = RfSetup(0);
        register.set_rf_pwr(power);

        let (dr_low, dr_high) = match *rate {
            DataRate::R250Kbps => (true, false),
            DataRate::R1Mbps => (false, false),
            DataRate::R2Mbps => (false, true),
        };
        register.set_rf_dr_low(dr_low);
        register.set_rf_dr_high(dr_high);

        self.device().write_register(register)?;
        Ok(())
    }

    /// Set CRC mode
    fn set_crc(
        &mut self,
        mode: CrcMode,
    ) -> Result<(), D::Error> {
         self.device().update_config(|config| mode.set_config(config))
    }

    /// Sets the interrupt mask
    /// 
    /// When an interrupt mask is set to true, the interrupt is masked and 
    /// will not fire on the IRQ pin. When set to false, it will trigger the IRQ pin.
    fn set_interrupt_mask(
        &mut self,
        data_ready_rx: bool,
        data_sent_tx: bool,
        max_retransmits_tx: bool
    ) -> Result<(), D::Error> {
        self.device().update_config(|config| {
            config.set_mask_rx_dr(data_ready_rx);
            config.set_mask_tx_ds(data_sent_tx);
            config.set_mask_max_rt(max_retransmits_tx);
        })
    }

    /// Configure which RX pipes to enable
    fn set_pipes_rx_enable(
        &mut self,
        bools: &[bool; PIPES_COUNT],
    ) -> Result<(), D::Error> {
        self.device().write_register(EnRxaddr::from_bools(bools))?;
        Ok(())
    }

    /// Set address `addr` of pipe number `pipe_no`
    fn set_rx_addr(
        &mut self,
        pipe_no: usize,
        addr: &[u8],
    ) -> Result<(), D::Error> {
        macro_rules! w {
            ( $($no: expr, $name: ident);+ ) => (
                match pipe_no {
                    $(
                        $no => {
                            use crate::registers::$name;
                            let register = $name::new(addr);
                            self.device().write_register(register)?;
                        }
                    )+
                        _ => panic!("No such pipe {}", pipe_no)
                }
            )
        }
        w!(0, RxAddrP0;
           1, RxAddrP1;
           2, RxAddrP2;
           3, RxAddrP3;
           4, RxAddrP4;
           5, RxAddrP5);
        Ok(())
    }

    /// Set address of the TX pipe
    fn set_tx_addr(
        &mut self,
        addr: &[u8],
    ) -> Result<(), D::Error> {
        let register = TxAddr::new(addr);
        self.device().write_register(register)?;
        Ok(())
    }

    /// Configure auto-retransmit
    ///
    /// To disable, call as `set_auto_retransmit(0, 0)`.
    fn set_auto_retransmit(
        &mut self,
        delay: u8,
        count: u8,
    ) -> Result<(), D::Error> {
        self.device().write_register(auto_retransmit(delay, count))?;
        Ok(())
    }

    /// Obtain auto-acknowledgment configuration for all pipes
    fn get_auto_ack(
        &mut self,
    ) -> Result<[bool; PIPES_COUNT], D::Error> {
        // Read
        let (_, register) = self.device().read_register::<EnAa>()?;
        Ok(register.to_bools())
    }

    /// Configure auto-acknowledgment for all RX pipes
    ///
    /// TODO: handle switching tx/rx modes when auto-retransmit is enabled
    fn set_auto_ack(
        &mut self,
        bools: &[bool; PIPES_COUNT],
    ) -> Result<(), D::Error> {
        // Convert back
        let register = EnAa::from_bools(bools);
        // Write back
        self.device().write_register(register)?;
        Ok(())
    }

    /// Get address width configuration
    fn get_address_width(
        &mut self,
    ) -> Result<u8, D::Error> {
        let (_, register) = self.device().read_register::<SetupAw>()?;
        Ok(2 + register.aw())
    }

    /// Obtain interrupt pending status as `(RX_DR, TX_DR, MAX_RT)`
    /// where `RX_DR` indicates new data in the RX FIFO, `TX_DR`
    /// indicates that a packet has been sent, and `MAX_RT` indicates
    /// maximum retransmissions without auto-ack.
    fn get_interrupts(
        &mut self,
    ) -> Result<(bool, bool, bool), D::Error> {
        let (status, ()) = self.device().send_command(&Nop)?;
        Ok((status.rx_dr(), status.tx_ds(), status.max_rt()))
    }

    /// Clear interrupts, and return the interrupts set before clearing
    fn clear_interrupts(
        &mut self,
    ) -> Result<Interrupts, D::Error> {
        let mut clear = Status(0);
        clear.set_rx_dr(true);
        clear.set_tx_ds(true);
        clear.set_max_rt(true);
        // the contents of the status register is sent back to
        // us _before_ every operation so this returns the state of interrupts
        // before they are cleared.
        match self.device().write_register(clear) {
            Ok(status) => Ok(Interrupts {
                rx_dr: status.rx_dr(), 
                tx_ds: status.tx_ds(), 
                max_rt: status.max_rt() }),
            Err(e) => Err(e)
        }
    }

    /// ## `bools`
    /// * `None`: Dynamic payload length
    /// * `Some(len)`: Static payload length `len`
    fn set_pipes_rx_lengths(
        &mut self,
        lengths: &[Option<u8>; PIPES_COUNT],
    ) -> Result<(), D::Error> {
        // Enable dynamic payload lengths
        let mut bools = [true; PIPES_COUNT];
        for (i, length) in lengths.iter().enumerate() {
            bools[i] = length.is_none();
        }
        let dynpd = Dynpd::from_bools(&bools);
        if dynpd.0 != 0 {
            self.device().update_register::<Feature, _, _>(|feature| {
                feature.set_en_dpl(true);
            })?;
        }
        self.device().write_register(dynpd)?;

        // Set static payload lengths
        macro_rules! set_rx_pw {
            ($name: ident, $index: expr) => {{
                use crate::registers::$name;
                let length = lengths[$index].unwrap_or(0);
                let mut register = $name(0);
                register.set(length);
                self.device().write_register(register)?;
            }};
        }
        set_rx_pw!(RxPwP0, 0);
        set_rx_pw!(RxPwP1, 1);
        set_rx_pw!(RxPwP2, 2);
        set_rx_pw!(RxPwP3, 3);
        set_rx_pw!(RxPwP4, 4);
        set_rx_pw!(RxPwP5, 5);

        Ok(())
    }
}
