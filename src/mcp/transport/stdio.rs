use super::Transport;

#[allow(dead_code)]
pub struct Stdio;

impl Transport for Stdio {
    fn send(&mut self, data: &[u8]) -> Result<(), String> {
        use std::io::Write;
        std::io::stdout().write_all(data).map_err(|e| e.to_string())
    }

    fn receive(&mut self) -> Result<Vec<u8>, String> {
        use std::io::Read;
        let mut buffer = Vec::new();
        std::io::stdin()
            .read_to_end(&mut buffer)
            .map_err(|e| e.to_string())?;
        Ok(buffer)
    }
}
