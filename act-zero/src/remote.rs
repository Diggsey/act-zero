use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::Addr;

#[derive(Serialize, Deserialize)]
pub struct Remote<T>(T);

impl<T> Remote<T> {
    pub fn new(inner: T) -> Addr<Self> {
        Addr(Some(Arc::new(Remote(inner))))
    }
    pub fn inner(&self) -> &T {
        &self.0
    }
}
