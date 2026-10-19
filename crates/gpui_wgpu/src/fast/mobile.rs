//! Android's Goldfish Vulkan driver can crash while assigning debug object names.
//! Keep validation, but avoid debug-utils naming on creation and GPU recovery.

pub(crate) fn instance_flags() -> wgpu::InstanceFlags {
    let flags = wgpu::InstanceFlags::default();
    #[cfg(target_os = "android")]
    let flags = flags.difference(wgpu::InstanceFlags::DEBUG);
    flags
}
