mod stdio;

#[allow(dead_code)]
pub trait Transport {
    fn send(&mut self, data: &[u8]) -> Result<(), String>;
    fn receive(&mut self) -> Result<Vec<u8>, String>;
}
