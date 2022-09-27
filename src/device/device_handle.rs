use super::*;

pub struct RealDeviceHandle {
    pub(crate) handle: rusb::DeviceHandle<Context>
}
impl DeviceHandle for RealDeviceHandle {
    fn claim_interface(&mut self, iface: u8) -> Result<()> {
        Ok(self.handle.claim_interface(iface)?)
    }
    fn reset(&mut self) -> Result<()> {
        Ok(self.handle.reset()?)
    }

    fn read_control(
        &self,
        request_type: u8,
        request: u8,
        value: u16,
        index: u16,
        buf: &mut [u8],
        timeout: Duration,
    ) -> Result<usize> {
        Ok(self.handle.read_control(request_type, request, value, index, buf, timeout)?)
    }

    fn write_control(
        &self,
        request_type: u8,
        request: u8,
        value: u16,
        index: u16,
        buf: &[u8],
        timeout: Duration,
    ) -> Result<usize> {
        Ok(self.handle.write_control(request_type, request, value, index, buf, timeout)?)
    }

    fn read_bulk(
        &self,
        endpoint: u8,
        buf: &mut [u8],
        timeout: Duration,
    ) -> Result<usize> {
        Ok(self.handle.read_bulk(endpoint, buf, timeout)?)
    }
}
