use crate::resources::BufferHandle;

pub struct ReadBuffer(pub(crate) BufferHandle);

pub struct WriteBuffer(pub(crate) BufferHandle);

pub struct ReadWriteBuffer(pub(crate) BufferHandle);
