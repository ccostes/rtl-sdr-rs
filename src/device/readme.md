This module provides USB IO functionality. The structure is a little convoluted (potentially unnecessarily - plan to revisit) in order to support mocking for integration tests, which will be described here.

At a high level:
  * **Device**: public interface
  * **DeviceHandle**: Interface to a USB device handle - basically just takes the functions we use from `rusb`'s `DeviceHandle` and puts them in a `Trait` so that they can be mocked in the unit tests in [device_test.rs](device_test.rs).
  * **DeviceHandleBox**: A tuple struct that contains a boxed `DeviceHandle` (`pub struct DeviceHandleBox(pub Box<dyn DeviceHandle>)`)

`Device` uses a dependency-injection pattern, containing a `DeviceHandleBox` allowing a real or mock version to be used. For example, in unit tests we can use the `Mockall` crate to create a mocked `DeviceHandle`: `let mut mock_handle = MockDeviceHandle::new();` which can then be used when creating the `Device`: `let device = RealDevice{ handle: DeviceHandleBox(Box::new(mock_handle)) };`.