//! This module includes the [`BufferExt`] extension which allows creation of buffers with a [`Device`]
//! via the builder pattern.

use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{Buffer, BufferDescriptor, BufferUsages, Device, Label};

/// The extension trait which gives [`Device`] a method to create buffers using the builder pattern.
pub trait BufferExt {
    /// Create a buffer using the builder pattern.
    ///
    /// 1. First, call this.
    /// 2. Then, call any of the following methods in a chain, like so:
    /// ```rust
    /// device
    /// .buffer()
    /// // It is mandatory to use at least one of the following, and they must not disagree
    /// .size(n)
    /// .data(&[...])
    /// // The following are optional
    /// .label("foo")
    /// .uniform()
    /// .storage()
    /// .vertex()
    /// .index()
    /// .indirect()
    /// .query_resolve()
    /// .copy_src()
    /// .copy_dst()
    /// .map_read()
    /// .map_write()
    /// ```
    /// 3. Finally, call `.finish()` at the end of the chain. This will produce the buffer.
    ///
    /// See each method for more details on what settings they affect.
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

/// A builder that creates buffers.
///
/// See its methods or [`BufferExt::buffer()`](BufferExt::buffer()) for more information.
///
/// (`BufferExt` is implemented for [`wgpu::Device`])
pub struct BufferBuilder<'d, 'l, 'b> {
    device: &'d Device,
    label: Label<'l>,
    size: Option<u64>,
    data: Option<&'b [u8]>,
    usage: BufferUsages,
    mapped: bool,
}

impl<'d, 'l, 'b> BufferBuilder<'d, 'l, 'b> {
    /// Add a label to the buffer, which can be visible in debugging tools.
    pub fn label(mut self, label: &'l str) -> Self {
        self.label = Some(label);
        self
    }

    /// Set the size of the buffer. This is the number of bytes that the buffer can store.
    /// Either this method or [`data()`] must be used or the buffer will not have a definite size.
    /// If both are used, they must agree on how long the buffer is. There is no particular
    /// reason to use both, either is sufficient.
    pub fn size(mut self, size: u64) -> Self {
        self.size = Some(size);
        self
    }

    /// Set the buffer's contents. This will fill the buffer with the given bytes on creation.
    /// In order to do so, the buffer must be created mapped using the [`mapped()`] method.
    /// Note also that the buffer must be unmapped after creation in order to be used;
    /// call [`Buffer::unmap()`] to do so.
    ///
    /// Either this method or [`sized()`] is mandatory to create the buffer. There is no particular
    /// reason to use both, but if both are used, they must agree about the size of the buffer.
    pub fn data(mut self, data: &'b [u8]) -> Self {
        self.data = Some(data);
        self
    }

    /// Map the buffer on creation. This will allow the CPU to read and write to and from the buffer
    /// immediately, which otherwise must be done by manually mapping the buffer.
    pub fn mapped(mut self) -> Self {
        self.mapped = true;
        self
    }

    /// Allow the buffer to be used as a uniform buffer. This is commonly used for non-vertex readonly
    /// data in shaders. If you want a writeable buffer, or are writing a render pass and want to supply
    /// vertices, use [`storage()`] or [`vertex`] respectively. See also [`index()`] for index buffers,
    /// and [`indirect()`] for indirect buffers.
    pub fn uniform(mut self) -> Self {
        self.usage |= BufferUsages::UNIFORM;
        self
    }

    /// Allow the buffer to be used as a storage buffer. Storage buffers can be written to from shaders,
    /// which is very useful for compute shaders. If you only want to read from the buffer, consider
    /// using [`uniform()`] instead.
    pub fn storage(mut self) -> Self {
        self.usage |= BufferUsages::STORAGE;
        self
    }

    /// Allow the buffer to be used as a vertex buffer. Vertex buffers contain data for each of the vertices
    /// of a mesh. These are generally supplied to render passes to represent meshes, along with an
    /// index buffer (created using [`index()`]). Sometimes indirect buffers are used as well ([`indirect()`]).
    pub fn vertex(mut self) -> Self {
        self.usage |= BufferUsages::VERTEX;
        self
    }

    /// Index buffers contain a list of indices representing triangles of a mesh. A common way to represent
    /// triangles in an index buffer is to have every 3 indices form one triangle; Every index corresponds
    /// to a vertex in an associated vertex buffer. These are frequently used in render passes as an
    /// optimization/tool for flexibility.
    pub fn index(mut self) -> Self {
        self.usage |= BufferUsages::INDEX;
        self
    }

    /// Indirect buffers allow some amount of work to be moved from the CPU to the GPU. They generally
    /// store some collection of commands to be executed by the GPU. This allows complex actions such as
    /// determining the dispatch dimensions of a compute shader using another compute shader.
    ///
    /// Here are some useful links for indirect buffers:
    ///
    /// [`wgpu::RenderPass::draw_indirect()`]
    ///
    /// [`wgpu::RenderPass::draw_indexed_indirect`]
    ///
    /// [`wgpu::util::DrawIndirect`]
    ///
    /// [`wgpu::util::DrawIndexedIndirect`]
    pub fn indirect(mut self) -> Self {
        self.usage |= BufferUsages::INDIRECT;
        self
    }

    /// Allows using the buffer as a query resolve buffer. This is where the results of queries
    /// are stored. See [`wgpu::Device::create_query_set()`] and [`wgpu::QueryType`].
    pub fn query_resolve(mut self) -> Self {
        self.usage |= BufferUsages::QUERY_RESOLVE;
        self
    }

    /// Allows the buffer to be copied from, generally into another buffer but also potentially
    /// into a texture. See [`wgpu::CommandEncoder::copy_buffer_to_buffer`] and related methods.
    pub fn copy_src(mut self) -> Self {
        self.usage |= BufferUsages::COPY_SRC;
        self
    }

    /// Allows the buffer to be copied into, generally from another buffer but also potentially
    /// from a texture. See [`wgpu::CommandEncoder::copy_buffer_to_buffer`] and related methods.
    pub fn copy_dst(mut self) -> Self {
        self.usage |= BufferUsages::COPY_DST;
        self
    }

    /// Allows the buffer, if mapped manually, to be read from by the CPU.
    ///
    /// See [`BufferSlice::map_async`] for more information.
    /// Also see [this issue](https://github.com/gfx-rs/wgpu/discussions/1438).
    pub fn map_read(mut self) -> Self {
        self.usage |= BufferUsages::MAP_READ;
        self
    }

    /// Allows the buffer, if mapped manually, to be written to by the CPU.
    ///
    /// See [`BufferSlice::map_async`] for more information.
    /// Also see [this issue](https://github.com/gfx-rs/wgpu/discussions/1438).
    pub fn map_write(mut self) -> Self {
        self.usage |= BufferUsages::MAP_WRITE;
        self
    }

    /// Finish the buffer and create it.
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
