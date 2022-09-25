use mockall::predicate::{self, eq};

use crate::device::{MockDeviceHandle, RealDevice, DeviceHandleBox, EEPROM_SIZE};

use super::{Device, BLOCK_SYS, GPO, CTRL_IN, CTRL_TIMEOUT};

#[test]
fn test_read_reg_8_bit() {
    let block = BLOCK_SYS;
    let index_expected = BLOCK_SYS << 8;
    let addr = GPO;
    let data_expected = 0x12_u16;

    let mut mock_handle = MockDeviceHandle::new();
    mock_handle.expect_read_control()
        .times(1)
        .with(eq(CTRL_IN), eq(0), eq(addr), eq(index_expected), predicate::always(), eq(CTRL_TIMEOUT))
        .returning(move |_, _, _, _, data, _| {
            assert!(data.len() == 1);
            data[0] = data_expected as u8;
            Ok(1)
        });
    let device = RealDevice{
        handle: DeviceHandleBox(Box::new(mock_handle))};
    let result = device.read_reg(block, addr, 1).unwrap();
    assert_eq!(data_expected, result);
}

#[test]
fn test_read_reg_16_bit() {
    let block = BLOCK_SYS;
    let index_expected = BLOCK_SYS << 8;
    let addr = GPO;
    let data_expected = u16::to_be_bytes(0x123);

    let mut mock_handle = MockDeviceHandle::new();
    mock_handle.expect_read_control()
        .times(1)
        .with(eq(CTRL_IN), eq(0), eq(addr), eq(index_expected), predicate::always(), eq(CTRL_TIMEOUT))
        .returning(move |_, _, _, _, data, _| {
            data[0] = data_expected[0];
            data[1] = data_expected[1];
            Ok(2)
        });
    let device = RealDevice{
        handle: DeviceHandleBox(Box::new(mock_handle))};
    let result = device.read_reg(block, addr, 2).unwrap();
    assert_eq!(u16::from_be_bytes(data_expected), result);
}

#[test]
#[should_panic]
fn test_read_eeprom_out_of_range(){
    let mock_handle = MockDeviceHandle::new();
    let device = RealDevice{
        handle: DeviceHandleBox(Box::new(mock_handle))};
    let data = [0;5];
    // Try to read more than eeprom size - should panic
    device.read_eeprom(&data, 0, EEPROM_SIZE).unwrap();
}