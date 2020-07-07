pub trait SenderExt {
    type Item;
}

impl<T> SenderExt for crate::Sender<T> {
    type Item = T;
}
