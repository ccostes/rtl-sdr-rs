use mockall::predicate::{self, eq};

use crate::device::mock_device_handle::MockDeviceHandle;
use crate::device::{Device, EEPROM_SIZE};

use super::{BLOCK_SYS, CTRL_IN, CTRL_OUT, CTRL_TIMEOUT, GPO};

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
fn test_read_reg_u16() {
    let block = BLOCK_SYS;
    let index_expected = BLOCK_SYS << 8;
    let addr = GPO;
    let data_expected = u16::to_be_bytes(0x123);

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
    assert_eq!(u16::from_be_bytes(data_expected), result);
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
    let data = [0; 5];
    // Try to read more than eeprom size - should panic
    device.read_eeprom(&data, 0, EEPROM_SIZE).unwrap();
}
