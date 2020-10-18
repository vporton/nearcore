use crate::{Context, DeepSizeOf};
use actix::io::FramedWrite;
use actix::Message;

#[cfg(features = "slotmap")]
mod slotmap_impl {
    use super::*;

    known_deep_size!(0, slotmap::KeyData, slotmap::DefaultKey);

    impl<K, V> DeepSizeOf for slotmap::SlotMap<K, V>
    where
        K: DeepSizeOf + slotmap::Key,
        V: DeepSizeOf + slotmap::Slottable,
    {
        fn deep_size_of_children(&self, context: &mut Context) -> usize {
            self.iter().fold(0, |sum, (key, val)| {
                sum + key.deep_size_of_children(context) + val.deep_size_of_children(context)
            }) + self.capacity() * size_of::<(u32, V)>
                > ()
        }
    }
}

impl<T: actix::Actor> DeepSizeOf for actix::Addr<T> {
    fn deep_size_of_children(&self, _: &mut Context) -> usize {
        0
    }
}

impl<T: tokio::io::AsyncWrite + Unpin, U: tokio_util::codec::Encoder> DeepSizeOf
    for FramedWrite<T, U>
{
    fn deep_size_of_children(&self, _: &mut Context) -> usize {
        0
    }
}

impl<M: Message> DeepSizeOf for actix::Recipient<M>
where
    M: Message + Send,
    M::Result: Send,
{
    fn deep_size_of_children(&self, _: &mut Context) -> usize {
        0
    }
}
