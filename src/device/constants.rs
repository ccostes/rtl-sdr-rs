#![allow(dead_code)]

use std::time::Duration;

pub struct UsbDeviceSignature {
    pub vid: u16,
    pub pid: u16,
    pub description: &'static str,
}
pub const KNOWN_DEVICES: &'static [UsbDeviceSignature; 42] = &[
    UsbDeviceSignature {
        vid: 0x0bda,
        pid: 0x2832,
        description: "Generic RTL2832U",
    },
    UsbDeviceSignature {
        vid: 0x0bda,
        pid: 0x2838,
        description: "Generic RTL2832U OEM",
    },
    UsbDeviceSignature {
        vid: 0x0413,
        pid: 0x6680,
        description: "DigitalNow Quad DVB-T PCI-E card",
    },
    UsbDeviceSignature {
        vid: 0x0413,
        pid: 0x6f0f,
        description: "Leadtek WinFast DTV Dongle mini D",
    },
    UsbDeviceSignature {
        vid: 0x0458,
        pid: 0x707f,
        description: "Genius TVGo DVB-T03 USB dongle (Ver. B)",
    },
    UsbDeviceSignature {
        vid: 0x0ccd,
        pid: 0x00a9,
        description: "Terratec Cinergy T Stick Black (rev 1)",
    },
    UsbDeviceSignature {
        vid: 0x0ccd,
        pid: 0x00b3,
        description: "Terratec NOXON DAB/DAB+ USB dongle (rev 1)",
    },
    UsbDeviceSignature {
        vid: 0x0ccd,
        pid: 0x00b4,
        description: "Terratec Deutschlandradio DAB Stick",
    },
    UsbDeviceSignature {
        vid: 0x0ccd,
        pid: 0x00b5,
        description: "Terratec NOXON DAB Stick - Radio Energy",
    },
    UsbDeviceSignature {
        vid: 0x0ccd,
        pid: 0x00b7,
        description: "Terratec Media Broadcast DAB Stick",
    },
    UsbDeviceSignature {
        vid: 0x0ccd,
        pid: 0x00b8,
        description: "Terratec BR DAB Stick",
    },
    UsbDeviceSignature {
        vid: 0x0ccd,
        pid: 0x00b9,
        description: "Terratec WDR DAB Stick",
    },
    UsbDeviceSignature {
        vid: 0x0ccd,
        pid: 0x00c0,
        description: "Terratec MuellerVerlag DAB Stick",
    },
    UsbDeviceSignature {
        vid: 0x0ccd,
        pid: 0x00c6,
        description: "Terratec Fraunhofer DAB Stick",
    },
    UsbDeviceSignature {
        vid: 0x0ccd,
        pid: 0x00d3,
        description: "Terratec Cinergy T Stick RC (Rev.3)",
    },
    UsbDeviceSignature {
        vid: 0x0ccd,
        pid: 0x00d7,
        description: "Terratec T Stick PLUS",
    },
    UsbDeviceSignature {
        vid: 0x0ccd,
        pid: 0x00e0,
        description: "Terratec NOXON DAB/DAB+ USB dongle (rev 2)",
    },
    UsbDeviceSignature {
        vid: 0x1554,
        pid: 0x5020,
        description: "PixelView PV-DT235U(RN)",
    },
    UsbDeviceSignature {
        vid: 0x15f4,
        pid: 0x0131,
        description: "Astrometa DVB-T/DVB-T2",
    },
    UsbDeviceSignature {
        vid: 0x15f4,
        pid: 0x0133,
        description: "HanfTek DAB+FM+DVB-T",
    },
    UsbDeviceSignature {
        vid: 0x185b,
        pid: 0x0620,
        description: "Compro Videomate U620F",
    },
    UsbDeviceSignature {
        vid: 0x185b,
        pid: 0x0650,
        description: "Compro Videomate U650F",
    },
    UsbDeviceSignature {
        vid: 0x185b,
        pid: 0x0680,
        description: "Compro Videomate U680F",
    },
    UsbDeviceSignature {
        vid: 0x1b80,
        pid: 0xd393,
        description: "GIGABYTE GT-U7300",
    },
    UsbDeviceSignature {
        vid: 0x1b80,
        pid: 0xd394,
        description: "DIKOM USB-DVBT HD",
    },
    UsbDeviceSignature {
        vid: 0x1b80,
        pid: 0xd395,
        description: "Peak 102569AGPK",
    },
    UsbDeviceSignature {
        vid: 0x1b80,
        pid: 0xd397,
        description: "KWorld KW-UB450-T USB DVB-T Pico TV",
    },
    UsbDeviceSignature {
        vid: 0x1b80,
        pid: 0xd398,
        description: "Zaapa ZT-MINDVBZP",
    },
    UsbDeviceSignature {
        vid: 0x1b80,
        pid: 0xd39d,
        description: "SVEON STV20 DVB-T USB & FM",
    },
    UsbDeviceSignature {
        vid: 0x1b80,
        pid: 0xd3a4,
        description: "Twintech UT-40",
    },
    UsbDeviceSignature {
        vid: 0x1b80,
        pid: 0xd3a8,
        description: "ASUS U3100MINI_PLUS_V2",
    },
    UsbDeviceSignature {
        vid: 0x1b80,
        pid: 0xd3af,
        description: "SVEON STV27 DVB-T USB & FM",
    },
    UsbDeviceSignature {
        vid: 0x1b80,
        pid: 0xd3b0,
        description: "SVEON STV21 DVB-T USB & FM",
    },
    UsbDeviceSignature {
        vid: 0x1d19,
        pid: 0x1101,
        description: "Dexatek DK DVB-T Dongle (Logilink VG0002A)",
    },
    UsbDeviceSignature {
        vid: 0x1d19,
        pid: 0x1102,
        description: "Dexatek DK DVB-T Dongle (MSI DigiVox mini II V3.0)",
    },
    UsbDeviceSignature {
        vid: 0x1d19,
        pid: 0x1103,
        description: "Dexatek Technology Ltd. DK 5217 DVB-T Dongle",
    },
    UsbDeviceSignature {
        vid: 0x1d19,
        pid: 0x1104,
        description: "MSI DigiVox Micro HD",
    },
    UsbDeviceSignature {
        vid: 0x1f4d,
        pid: 0xa803,
        description: "Sweex DVB-T USB",
    },
    UsbDeviceSignature {
        vid: 0x1f4d,
        pid: 0xb803,
        description: "GTek T803",
    },
    UsbDeviceSignature {
        vid: 0x1f4d,
        pid: 0xc803,
        description: "Lifeview LV5TDeluxe",
    },
    UsbDeviceSignature {
        vid: 0x1f4d,
        pid: 0xd286,
        description: "MyGica TD312",
    },
    UsbDeviceSignature {
        vid: 0x1f4d,
        pid: 0xd803,
        description: "PROlectrix DV107669",
    },
];

pub const EEPROM_ADDR: u16 = 0xa0;
pub const EEPROM_SIZE: usize = 256;

// Blocks
pub const BLOCK_DEMOD: u16 = 0;
pub const BLOCK_USB: u16 = 1;
pub const BLOCK_SYS: u16 = 2;
pub const BLOCK_TUN: u16 = 3;
pub const BLOCK_ROM: u16 = 4;
pub const BLOCK_IRB: u16 = 5;
pub const BLOCK_IIC: u16 = 6;

// Sys Registers
pub const DEMOD_CTL: u16 = 0x3000;
pub const GPO: u16 = 0x3001;
pub const GPI: u16 = 0x3002;
pub const GPOE: u16 = 0x3003;
pub const GPD: u16 = 0x3004;
pub const SYSINTE: u16 = 0x3005;
pub const SYSINTS: u16 = 0x3006;
pub const GP_CFG0: u16 = 0x3007;
pub const GP_CFG1: u16 = 0x3008;
pub const SYSINTE_1: u16 = 0x3009;
pub const SYSINTS_1: u16 = 0x300a;
pub const DEMOD_CTL_1: u16 = 0x300b;
pub const IR_SUSPEND: u16 = 0x300c;

// USB Registers
pub const USB_SYSCTL: u16 = 0x2000;
pub const USB_CTRL: u16 = 0x2010;
pub const USB_STAT: u16 = 0x2014;
pub const USB_EPA_CFG: u16 = 0x2144;
pub const USB_EPA_CTL: u16 = 0x2148;
pub const USB_EPA_MAXPKT: u16 = 0x2158;
pub const USB_EPA_MAXPKT_2: u16 = 0x215a;
pub const USB_EPA_FIFO_CFG: u16 = 0x2160;

pub const CTRL_IN: u8 =
    rusb::constants::LIBUSB_ENDPOINT_IN | rusb::constants::LIBUSB_REQUEST_TYPE_VENDOR;
pub const CTRL_OUT: u8 =
    rusb::constants::LIBUSB_ENDPOINT_OUT | rusb::constants::LIBUSB_REQUEST_TYPE_VENDOR;
pub const CTRL_TIMEOUT: Duration = Duration::from_millis(300);
