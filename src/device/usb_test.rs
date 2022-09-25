use crate::device::{MockDeviceHandle, RealDevice, DeviceHandleBox, EEPROM_SIZE};

use super::Device;

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