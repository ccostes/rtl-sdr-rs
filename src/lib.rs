
use rusb::{Context, Device, DeviceHandle, Result, UsbContext, Error};
mod usb;
use usb::RtlSdrDeviceHandle;

mod tuners;
use tuners::*;

const VID: u16 = 0x0bda;
const PID: u16 = 0x2838;
const INTERFACE_ID: u8 = 0;

const FIR_LEN: usize = 16;
const DEFAULT_FIR: &'static [i32; FIR_LEN] = &[
    -54, -36, -41, -40, -32, -14, 14, 53,   // i8
    101, 156, 215, 273, 327, 372, 404, 421  // i12
];

const KNOWN_TUNERS: [TunerInfo; 1] = [r820t::TUNER_INFO];


pub struct RtlSdrDevice {
    handle: RtlSdrDeviceHandle,

}

impl RtlSdrDevice {
    pub fn open() -> RtlSdrDevice {
        let mut context = Context::new().unwrap();
        let (mut device, mut handle) = 
            RtlSdrDevice::open_device(&mut context, VID, PID).expect("Failed to open USB device");
        let mut device_handle = RtlSdrDeviceHandle::new(handle);
        device_handle.print_device_info();
        let mut device = RtlSdrDevice { handle: device_handle };
        device.init();
        device
    }

    fn open_device<T: UsbContext> (
        context: &mut T,
        vid: u16,
        pid: u16,
    ) -> Option<(Device<T>, DeviceHandle<T>)> {
        let devices = match context.devices() {
            Ok(d) => d,
            Err(_) => return None,
        };

        for device in devices.iter() {
            let device_desc = match device.device_descriptor() {
                Ok(d) => d,
                Err(_) => return None,
            };
            
            if device_desc.vendor_id() == vid && device_desc.product_id() == pid {
                match device.open() {
                    Ok(handle) => return Some((device, handle)),
                    Err(_) => continue,
                }
            }
        }
        None
    }

    fn init(&mut self) {
        self.power_on();
        self.set_i2c_repeater(true);
        
        let tuner = {
            let tuner_id = match self.search_tuner() {
                Some(tid) => {
                    println!("Got tuner ID {}", tid);
                    tid
                }
                None => {
                    panic!("Failed to find tuner, aborting");
                }
            };
            match tuner_id {
                R820T_TUNER_ID => r820t::R820T::new(&mut self.handle),
                _ => panic!("Unable to find recognized tuner"),
            }
        };
    
        // Finished Init
        self.set_i2c_repeater(false);
    }

    fn power_on(&mut self) {
        self.handle.claim_interface(INTERFACE_ID);

        self.handle.test_write();

        // Init baseband
        // println!("Initialize USB");
        self.handle.write_reg(usb::BLOCK_USB, usb::USB_SYSCTL, 0x09, 1);
        self.handle.write_reg(usb::BLOCK_USB, usb::USB_EPA_MAXPKT, 0x0002, 2);
        self.handle.write_reg(usb::BLOCK_USB, usb::USB_EPA_CTL, 0x1002, 2);

        // println!("Power-on demod");
        self.handle.write_reg(usb::BLOCK_SYS, usb::DEMOD_CTL_1, 0x22, 1);
        self.handle.write_reg(usb::BLOCK_SYS, usb::DEMOD_CTL, 0xe8, 1);

        // println!("Reset demod (bit 3, soft_rst)");
        self.handle.reset_demod();

        // println!("Disable spectrum inversion and adjust channel rejection");
        self.handle.demod_write_reg(1, 0x15, 0x00, 1);
        self.handle.demod_write_reg(1, 0x16, 0x00, 2);

        // println!("Clear DDC shift and IF registers");
        for i in 0..5 {
            self.handle.demod_write_reg(1, 0x16 + i, 0x00, 1);
        }
        self.set_fir(DEFAULT_FIR);

        // println!("Enable SDR mode, disable DAGC (bit 5)");
        self.handle.demod_write_reg(0, 0x19, 0x05, 1);

        // println!("Init FSM state-holding register");
        self.handle.demod_write_reg(1, 0x93, 0xf0, 1);
        self.handle.demod_write_reg(1, 0x94, 0x0f, 1);

        // Disable AGC (en_dagc, bit 0) (seems to have no effect)
        self.handle.demod_write_reg(1, 0x11, 0x00, 1);

        // Disable RF and IF AGC loop
        self.handle.demod_write_reg(1, 0x04, 0x00, 1);
        
        // Disable PID filter
        self.handle.demod_write_reg(0, 0x61, 0x60, 1);
        
        // opt_adc_iq = 0, default ADC_I/ADC_Q datapath
        self.handle.demod_write_reg(0, 0x06, 0x80, 1);
        
        // Enable Zero-IF mode, DC cancellation, and IQ estimation/compensation
        self.handle.demod_write_reg(1, 0xb1, 0x1b, 1);
        
        // Disable 4.096 MHz clock output on pin TP_CK0
        self.handle.demod_write_reg(0, 0x0d, 0x83, 1);
    }

    fn set_i2c_repeater(&mut self, enable: bool) {
        let val = match enable {
            true    => 0x18,
            false   => 0x10, 
        };
        self.handle.demod_write_reg(1, 0x01, val, 1);
    }


    pub fn set_fir(&mut self, fir: &[i32; FIR_LEN]) {
        const TMP_LEN: usize = 20;
        let mut tmp: [u8; TMP_LEN] = [0;TMP_LEN];

        for i in 0..7 {
            let val = fir[i];
            if val < -128 || val > 127 {
                panic!("i8 FIR coefficient out of bounds! {}", val);
            }
            tmp[i] = val as u8;
        }
        for i in (0..7).step_by(2) {
            let val0 = fir[8+i];
            let val1 = fir[8+i+1];
            if val0 < -2048 || val0 > 2047 {
                panic!("i12 FIR coefficient out of bounds: {}", val0)
            } else if val1 < -2048 || val1 > 2047 {
                panic!("i12 FIR coefficient out of bounds: {}", val1)
            }
            tmp[8 + i * 3 / 2] = (val0 >> 4) as u8;
            tmp[8 + i * 3 / 2 + 1] = ((val0 << 4) | ((val1 >> 8) & 0x0f)) as u8;
            tmp[8 + i * 3 / 2 + 2] = val1 as u8;
        }

        for i in 0..TMP_LEN - 1 {
            self.handle.demod_write_reg(1, 0x1c + 1, tmp[i] as u16, 1);
        }
    }

    fn search_tuner(&mut self) -> Option<&str> {
        for tuner_info in KNOWN_TUNERS.iter() {
            let regval = self.handle.i2c_read_reg(tuner_info.i2c_addr, tuner_info.check_addr);
            println!("Probing I2C address {:#02x} checking address {:#02x}", tuner_info.i2c_addr, tuner_info.check_addr);
            match regval {
                Ok(val) => {
                    // println!("Expecting value {:#02x}, got value {:#02x}", tuner_info.check_val, val);
                    if val == tuner_info.check_val {
                        return Some(tuner_info.id);
                    }
                }
                Err(e) => {
                    println!("Reading failed, continuing. Err: {}", e);
                }
            };
        }
        None
    }

}