use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{Buffer, BufferDescriptor, BufferUsages, Device, Label};

pub trait BufferExt {
    fn buffer(&self) -> BufferBuilder;
}

impl BufferExt for Device {
    fn buffer(&self) -> BufferBuilder {
        BufferBuilder {
            device: self,
            label: None,
            size: None,
            data: None,
            usage: BufferUsages::empty(),
            mapped: false,
        }
    }
}

pub struct BufferBuilder<'d, 'l, 'b> {
    device: &'d Device,
    label: Label<'l>,
    size: Option<u64>,
    data: Option<&'b [u8]>,
    usage: BufferUsages,
    mapped: bool,
}

impl<'d, 'l, 'b> BufferBuilder<'d, 'l, 'b> {
    pub fn label(mut self, label: &'l str) -> Self {
        self.label = Some(label);
        self
    }

    pub fn size(mut self, size: u64) -> Self {
        self.size = Some(size);
        self
    }

    pub fn data(mut self, data: &'b [u8]) -> Self {
        self.data = Some(data);
        self
    }

    pub fn mapped(mut self) -> Self {
        self.mapped = true;
        self
    }

    pub fn uniform(mut self) -> Self {
        self.usage |= BufferUsages::UNIFORM;
        self
    }

    pub fn storage(mut self) -> Self {
        self.usage |= BufferUsages::STORAGE;
        self
    }

    pub fn vertex(mut self) -> Self {
        self.usage |= BufferUsages::VERTEX;
        self
    }

    pub fn index(mut self) -> Self {
        self.usage |= BufferUsages::INDEX;
        self
    }

    pub fn indirect(mut self) -> Self {
        self.usage |= BufferUsages::INDIRECT;
        self
    }

    pub fn query_resolve(mut self) -> Self {
        self.usage |= BufferUsages::QUERY_RESOLVE;
        self
    }

    pub fn copy_src(mut self) -> Self {
        self.usage |= BufferUsages::COPY_SRC;
        self
    }

    pub fn copy_dst(mut self) -> Self {
        self.usage |= BufferUsages::COPY_DST;
        self
    }

    pub fn map_read(mut self) -> Self {
        self.usage |= BufferUsages::MAP_READ;
        self
    }

    pub fn map_write(mut self) -> Self {
        self.usage |= BufferUsages::MAP_WRITE;
        self
    }

    pub fn finish(self) -> Buffer {
        let Self {
            device,
            label,
            size,
            data,
            usage,
            mapped,
        } = self;

        assert!(
            (size.is_none() || data.is_none()) || size.unwrap() as usize == data.unwrap().len(),
            "size and data length do not match"
        );
        assert!(
            size.is_some() || data.is_some(),
            "must provide at least one of size or data to create a buffer"
        );
        assert!(
            data.is_none() || mapped,
            "if data is provided, the buffer must be mapped at creation"
        );

        match data {
            Some(data) => device.create_buffer_init(&BufferInitDescriptor {
                label,
                contents: data,
                usage,
            }),
            None => device.create_buffer(&BufferDescriptor {
                label,
                size: size.unwrap(),
                usage,
                mapped_at_creation: mapped,
            }),
        }
    }
}
