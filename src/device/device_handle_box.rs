use super::*;

pub struct DeviceHandleBox(pub Box<dyn DeviceHandle>);
impl DeviceHandle for DeviceHandleBox {
    fn claim_interface(&mut self, iface: u8) -> Result<()> {
        self.0.claim_interface(iface)
    }
    fn reset(&mut self) -> Result<()> {
        self.0.reset()
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
        self.0.read_control(request_type, request, value, index, buf, timeout)
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
        self.0.write_control(request_type, request, value, index, buf, timeout)
    }
    fn read_bulk(
            &self,
            endpoint: u8,
            buf: &mut [u8],
            timeout: Duration,
        ) -> Result<usize> {
        self.0.read_bulk(endpoint, buf, timeout)
    }
}