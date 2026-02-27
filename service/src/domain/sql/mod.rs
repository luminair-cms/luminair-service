pub mod modify;
pub mod query;

#[derive(Clone,Copy,Debug)]
pub struct SqlParameterRef {
    index: usize
}

impl From<usize> for SqlParameterRef {
    fn from(value: usize) -> Self {
        Self { index: value }
    }
}

impl SqlParameterRef {
    pub fn index(&self) -> usize {
        self.index
    }
}