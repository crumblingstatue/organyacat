pub(crate) struct ReadCursor<'a>(pub &'a [u8]);

impl<'a> ReadCursor<'a> {
    pub fn next_bytes<const N: usize>(&mut self) -> Option<&'a [u8; N]> {
        let (bytes, next) = self.0.split_first_chunk()?;
        self.0 = next;
        Some(bytes)
    }
    pub const fn next_n_bytes(&mut self, n: usize) -> &'a [u8] {
        let (bytes, next) = self.0.split_at(n);
        self.0 = next;
        bytes
    }
    pub fn next_u8(&mut self) -> Option<u8> {
        let (&byte, next) = self.0.split_first()?;
        self.0 = next;
        Some(byte)
    }
    pub fn next_u16_le(&mut self) -> Option<u16> {
        self.next_bytes().copied().map(u16::from_le_bytes)
    }
    pub fn next_u32_le(&mut self) -> Option<u32> {
        self.next_bytes().copied().map(u32::from_le_bytes)
    }
    pub fn u8_at(&self, offset: usize) -> u8 {
        self.0[offset]
    }
    pub fn u32_le_at(&self, offset: usize) -> u32 {
        u32::from_le_bytes(*self.0[offset..].first_chunk().unwrap())
    }
    pub fn skip(&mut self, amount: usize) {
        self.0 = &self.0[amount..];
    }
}
