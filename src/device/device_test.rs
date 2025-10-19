// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use mockall::predicate::{self, eq};

use crate::device::mock_device_handle::MockDeviceHandle;
use crate::device::{Device, EEPROM_SIZE};
use crate::DeviceId;

use super::{BLOCK_IIC, BLOCK_SYS, CTRL_IN, CTRL_OUT, CTRL_TIMEOUT, EEPROM_ADDR, GPO};

#[test]
fn test_read_reg_u8() {
    let block = BLOCK_SYS;
    let index_expected = BLOCK_SYS << 8;
    let addr = GPO;
    let data_expected = 0x12_u16;

    let mut mock_handle = MockDeviceHandle::new();
    mock_handle
        .expect_read_control()
        .times(1)
        .with(
            eq(CTRL_IN),
            eq(0),
            eq(addr),
            eq(index_expected),
            predicate::always(),
            eq(CTRL_TIMEOUT),
        )
        .returning(move |_, _, _, _, data, _| {
            assert!(data.len() == 1);
            data[0] = data_expected as u8;
            Ok(1)
        });
    let device = Device {
        handle: mock_handle,
    };
    let result = device.read_reg(block, addr, 1).unwrap();
    assert_eq!(data_expected, result);
}

#[test]
fn test_usb_strings_delegates_to_handle() {
    let mut mock_handle = MockDeviceHandle::new();
    mock_handle
        .expect_get_usb_strings()
        .returning(|| Ok((Some("Make".to_string()), Some("Model".to_string()), None)));

    let device = Device {
        handle: mock_handle,
    };

    let (manufact, product, serial) = device.usb_strings().unwrap();
    assert_eq!(manufact.as_deref(), Some("Make"));
    assert_eq!(product.as_deref(), Some("Model"));
    assert!(serial.is_none());
}

#[test]
fn test_read_reg_u16() {
    let block = BLOCK_SYS;
    let index_expected = BLOCK_SYS << 8;
    let addr = GPO;
    // Bytes are read as little-endian
    let data_expected = u16::to_le_bytes(0x123);

    let mut mock_handle = MockDeviceHandle::new();
    mock_handle
        .expect_read_control()
        .times(1)
        .with(
            eq(CTRL_IN),
            eq(0),
            eq(addr),
            eq(index_expected),
            predicate::always(),
            eq(CTRL_TIMEOUT),
        )
        .returning(move |_, _, _, _, data, _| {
            data[0] = data_expected[0];
            data[1] = data_expected[1];
            Ok(2)
        });
    let device = Device {
        handle: mock_handle,
    };
    let result = device.read_reg(block, addr, 2).unwrap();
    assert_eq!(u16::from_le_bytes(data_expected), result);
}

#[test]
fn test_write_reg_u8() {
    let block = BLOCK_SYS;
    let index_expected = (BLOCK_SYS << 8) | 0x10;
    let addr = GPO;
    let data_expected = 0xef_u16;

    let mut mock_handle = MockDeviceHandle::new();
    mock_handle
        .expect_write_control()
        .times(1)
        .with(
            eq(CTRL_OUT),
            eq(0),
            eq(addr),
            eq(index_expected),
            predicate::always(),
            eq(CTRL_TIMEOUT),
        )
        .returning(move |_, _, _, _, data, _| {
            assert!(data.len() == 1);
            assert_eq!(data[0], data_expected as u8);
            Ok(1)
        });
    let device = Device {
        handle: mock_handle,
    };
    let result = device.write_reg(block, addr, data_expected, 1).unwrap();
    assert_eq!(1, result);
}

#[test]
fn test_write_reg_u16() {
    let block = BLOCK_SYS;
    let index_expected = (BLOCK_SYS << 8) | 0x10;
    let addr = GPO;
    let data_expected = 0xefab_u16;

    let mut mock_handle = MockDeviceHandle::new();
    mock_handle
        .expect_write_control()
        .times(1)
        .with(
            eq(CTRL_OUT),
            eq(0),
            eq(addr),
            eq(index_expected),
            predicate::always(),
            eq(CTRL_TIMEOUT),
        )
        .returning(move |_, _, _, _, data, _| {
            assert!(data.len() == 2);
            assert_eq!(data, data_expected.to_be_bytes());
            Ok(1)
        });
    let device = Device {
        handle: mock_handle,
    };
    let result = device.write_reg(block, addr, data_expected, 2).unwrap();
    assert_eq!(1, result);
}

#[test]
fn test_demod_read_reg() {
    let page = 0xa_u16;
    let addr = 0x1_u16;
    let value = 0x12;

    let mut mock_handle = MockDeviceHandle::new();
    mock_handle
        .expect_read_control()
        .times(1)
        .with(
            eq(CTRL_IN),
            eq(0),
            eq((addr << 8) | 0x20),
            eq(page),
            predicate::always(),
            eq(CTRL_TIMEOUT),
        )
        .returning(move |_, _, _, _, data, _| {
            data[0] = value;
            Ok(2)
        });
    let device = Device {
        handle: mock_handle,
    };
    let result = device.demod_read_reg(page, addr).unwrap();
    assert_eq!(value as u16, result);
}

#[test]
#[should_panic]
fn test_read_eeprom_out_of_range() {
    let mock_handle = MockDeviceHandle::new();
    let device = Device {
        handle: mock_handle,
    };
    let mut data = [0; 5];
    // Try to read more than eeprom size - should panic
    device.read_eeprom(&mut data, 0, EEPROM_SIZE).unwrap();
}

#[test]
fn test_read_eeprom_reads_expected_data() {
    let mut mock_handle = MockDeviceHandle::new();

    // Expect the write_control call for setting the offset
    mock_handle
        .expect_write_control()
        .times(1)
        .with(
            eq(CTRL_OUT),                // Direction of the control transfer
            eq(0),                       // Request value (typically 0 for these operations)
            eq(EEPROM_ADDR),             // The address being accessed
            eq((BLOCK_IIC << 8) | 0x10), // Index value
            eq([0]),                     // Data being written, setting the offset
            eq(CTRL_TIMEOUT),            // Timeout value
        )
        .returning(|_, _, _, _, _, _| Ok(1)); // Return success

    // Expect the read_control call for reading the data
    let expected_data = [0x12, 0x34, 0x56, 0x78, 0x9A];
    mock_handle
        .expect_read_control()
        .times(expected_data.len()) // This will be called len times
        .returning(move |_, _, _, _, buf, _| {
            static mut CALL_COUNT: usize = 0;
            let call_count = unsafe { CALL_COUNT };
            buf[0] = expected_data[call_count];
            unsafe { CALL_COUNT += 1 };
            Ok(1) // Return success
        });

    let device = Device {
        handle: mock_handle,
    };
    let mut data = [0; 5];
    let data_len = data.len();
    device.read_eeprom(&mut data, 0, data_len).unwrap();
    assert_eq!(data, expected_data);
}

#[test]
fn test_read_eeprom_partial_read() {
    let mut mock_handle = MockDeviceHandle::new();

    // Mock the write_control call in write_array
    mock_handle
        .expect_write_control()
        .times(1)
        .with(
            eq(CTRL_OUT),
            eq(0),
            eq(EEPROM_ADDR),
            eq((BLOCK_IIC << 8) | 0x10),
            eq([0]), // Setting the offset to 0
            eq(CTRL_TIMEOUT),
        )
        .returning(|_, _, _, _, _, _| Ok(1));

    // Mock the read_control call in read_array
    let expected_data = [0xAB, 0xCD];
    mock_handle
        .expect_read_control()
        .times(expected_data.len()) // Expecting 2 calls, one for each byte
        .returning(move |_, _, _, _, buf, _| {
            static mut CALL_COUNT: usize = 0;
            let call_count = unsafe { CALL_COUNT };
            buf[0] = expected_data[call_count];
            unsafe { CALL_COUNT += 1 };
            Ok(1)
        });

    let device = Device {
        handle: mock_handle,
    };
    let mut data = [0; 2];
    let data_len = data.len();
    device.read_eeprom(&mut data, 0, data_len).unwrap();
    assert_eq!(data, expected_data);
}

#[test]
fn test_read_eeprom_larger_buffer() {
    let mut mock_handle = MockDeviceHandle::new();

    // Mock the write_control call in write_array
    mock_handle
        .expect_write_control()
        .times(1)
        .with(
            eq(CTRL_OUT),
            eq(0),
            eq(EEPROM_ADDR),
            eq((BLOCK_IIC << 8) | 0x10),
            eq([0]), // Setting the offset to 0
            eq(CTRL_TIMEOUT),
        )
        .returning(|_, _, _, _, _, _| Ok(1));

    // Mock the read_control call in read_array
    let expected_data = [0xDE, 0xAD];
    mock_handle
        .expect_read_control()
        .times(expected_data.len()) // Expecting 2 calls, one for each byte
        .returning(move |_, _, _, _, buf, _| {
            static mut CALL_COUNT: usize = 0;
            let call_count = unsafe { CALL_COUNT };
            buf[0] = expected_data[call_count];
            unsafe { CALL_COUNT += 1 };
            Ok(1)
        });

    let device = Device {
        handle: mock_handle,
    };
    let mut data = [0xFF; 4];
    device.read_eeprom(&mut data, 0, 2).unwrap(); // Reading only 2 bytes
    assert_eq!(data[..2], expected_data); // Verify the first 2 bytes
    assert_eq!(data[2..], [0xFF, 0xFF]); // Verify that the rest remain unchanged
}

#[test]
#[should_panic]
fn test_read_eeprom_invalid_offset() {
    let mock_handle = MockDeviceHandle::new();
    let device = Device {
        handle: mock_handle,
    };
    let mut data = [0; 5];
    let data_len = data.len();
    // This should panic because the offset + length exceeds EEPROM_SIZE
    device
        .read_eeprom(&mut data, EEPROM_SIZE as u8, data_len)
        .unwrap();
}

#[test]
fn test_device_id_enum_variants() {
    // Test that we can create DeviceId variants
    let index_device_id = DeviceId::Index(0);
    let fd_device_id = DeviceId::Fd(42);

    // Test equality
    assert_eq!(index_device_id, DeviceId::Index(0));
    assert_eq!(fd_device_id, DeviceId::Fd(42));
    assert_ne!(index_device_id, fd_device_id);

    // Test debug representation
    assert_eq!(format!("{:?}", index_device_id), "Index(0)");
    assert_eq!(format!("{:?}", fd_device_id), "Fd(42)");
}
