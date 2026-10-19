//! Android GPU compatibility uses the shared renderer's existing texture instance path.
//! GLES 3.0 lacks compute/storage-buffer limits; Vulkan retains native requirements.

use crate::WgpuBackend;

/// An Android native window can be connected to only one graphics API at once.
/// Creating Vulkan and EGL surfaces for the same window either fails with
/// EGL_BAD_ALLOC or disrupts Vulkan's buffer queue. Pick before creating a surface.
#[cfg(not(target_family = "wasm"))]
pub(crate) fn instance(
    display: Option<Box<dyn wgpu::wgt::WgpuHasDisplayHandle>>,
) -> wgpu::Instance {
    #[cfg(target_os = "android")]
    {
        let instance = create_instance(wgpu::Backends::VULKAN, Some(Box::new(AndroidDisplay)));
        let adapters = gpui::block_on(instance.enumerate_adapters(wgpu::Backends::VULKAN));
        if adapters.iter().any(|adapter| {
            let info = adapter.get_info();
            if emulator_vulkan_driver(&info.name, &info.driver, &info.driver_info) {
                // Goldfish/ranchu can create a device but lose it on atlas creation.
                // Keep physical Vulkan devices on the native path; use GLES in AVDs.
                log::warn!(
                    "Using GLES for unstable Android emulator Vulkan driver: {}",
                    info.name
                );
                false
            } else {
                true
            }
        }) {
            return instance;
        }
        log::info!("Android Vulkan unavailable or excluded; creating a GLES-only native instance");
        create_instance(wgpu::Backends::GL, display)
    }
    #[cfg(not(target_os = "android"))]
    create_instance(wgpu::Backends::VULKAN | wgpu::Backends::GL, display)
}

#[cfg(any(target_os = "android", test))]
fn emulator_vulkan_driver(name: &str, driver: &str, driver_info: &str) -> bool {
    [name, driver, driver_info].iter().any(|value| {
        let value = value.to_ascii_lowercase();
        value.contains("goldfish") || value.contains("ranchu")
    })
}

#[cfg(not(target_family = "wasm"))]
fn create_instance(
    backends: wgpu::Backends,
    display: Option<Box<dyn wgpu::wgt::WgpuHasDisplayHandle>>,
) -> wgpu::Instance {
    wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends,
        flags: instance_flags(),
        backend_options: Default::default(),
        memory_budget_thresholds: Default::default(),
        display,
    })
}

#[cfg(target_os = "android")]
#[derive(Debug)]
struct AndroidDisplay;
#[cfg(target_os = "android")]
impl raw_window_handle::HasDisplayHandle for AndroidDisplay {
    fn display_handle(
        &self,
    ) -> Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError> {
        // Android's display handle is a unit marker; no native object is borrowed.
        Ok(unsafe {
            raw_window_handle::DisplayHandle::borrow_raw(
                raw_window_handle::RawDisplayHandle::Android(
                    raw_window_handle::AndroidDisplayHandle::new(),
                ),
            )
        })
    }
}

#[cfg(not(target_family = "wasm"))]
pub(crate) fn instance_flags() -> wgpu::InstanceFlags {
    let flags = wgpu::InstanceFlags::default();
    // Goldfish's Vulkan driver can crash inside debug-utils object naming.
    // Retain validation, but omit labels on initial creation and device recovery.
    #[cfg(target_os = "android")]
    let flags = flags.difference(wgpu::InstanceFlags::DEBUG);
    flags
}

#[cfg(not(target_family = "wasm"))]
pub(crate) fn required_limits(adapter: &wgpu::Adapter) -> wgpu::Limits {
    limits_for_backend(
        adapter.get_info().backend,
        adapter.limits(),
        cfg!(any(target_family = "wasm", target_os = "android")),
    )
}

#[cfg(not(target_family = "wasm"))]
fn limits_for_backend(
    backend: wgpu::Backend,
    available: wgpu::Limits,
    compatible_target: bool,
) -> wgpu::Limits {
    let limits = if compatible_target && backend == wgpu::Backend::Gl {
        wgpu::Limits::downlevel_webgl2_defaults()
    } else {
        wgpu::Limits::downlevel_defaults()
    };
    limits
        .using_resolution(available.clone())
        .using_alignment(available)
}

pub(crate) fn uses_texture_instance_data(backend: WgpuBackend) -> bool {
    texture_instance_data(
        backend,
        cfg!(any(target_family = "wasm", target_os = "android")),
    )
}

fn texture_instance_data(backend: WgpuBackend, compatible_target: bool) -> bool {
    compatible_target
        && matches!(
            backend,
            WgpuBackend::Gl | WgpuBackend::Native(wgpu::Backend::Gl)
        )
}

pub(crate) fn prefer_rgba_atlas(
    adapter: &wgpu::Adapter,
    available: wgpu::TextureUsages,
    required: wgpu::TextureUsages,
) -> bool {
    cfg!(any(target_family = "wasm", target_os = "android"))
        && adapter.get_info().backend == wgpu::Backend::Gl
        && available.contains(required)
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use super::{emulator_vulkan_driver, limits_for_backend, texture_instance_data};
    use crate::WgpuBackend;

    #[test]
    fn android_gles_accepts_a_non_compute_adapter() {
        let available = wgpu::Limits::downlevel_webgl2_defaults();
        let requested = limits_for_backend(wgpu::Backend::Gl, available.clone(), true);
        assert!(requested.check_limits(&available));
        assert_eq!(requested.max_compute_workgroups_per_dimension, 0);
        assert_eq!(requested.max_storage_buffers_per_shader_stage, 0);
        assert!(texture_instance_data(WgpuBackend::Gl, true));
        assert!(texture_instance_data(
            WgpuBackend::Native(wgpu::Backend::Gl),
            true
        ));
    }

    #[test]
    fn native_vulkan_and_desktop_gl_keep_native_requirements() {
        let available = wgpu::Limits::downlevel_defaults();
        for (backend, compatible) in [(wgpu::Backend::Vulkan, true), (wgpu::Backend::Gl, false)] {
            let requested = limits_for_backend(backend, available.clone(), compatible);
            assert_eq!(
                requested.max_compute_workgroups_per_dimension,
                available.max_compute_workgroups_per_dimension
            );
            assert_eq!(
                requested.max_storage_buffers_per_shader_stage,
                available.max_storage_buffers_per_shader_stage
            );
        }
        assert!(!texture_instance_data(
            WgpuBackend::Native(wgpu::Backend::Vulkan),
            true
        ));
        assert!(!texture_instance_data(WgpuBackend::Gl, false));
        assert!(!texture_instance_data(
            WgpuBackend::Native(wgpu::Backend::Gl),
            false
        ));
    }
    #[test]
    fn only_known_android_emulator_vulkan_identities_are_excluded() {
        assert!(emulator_vulkan_driver(
            "Goldfish GFXStream (Apple M4 Max)",
            "",
            ""
        ));
        assert!(emulator_vulkan_driver("Android GPU", "ranchu", "1.3"));
        assert!(!emulator_vulkan_driver(
            "Adreno (TM) 750",
            "Qualcomm",
            "Vulkan 1.3"
        ));
        assert!(!emulator_vulkan_driver("Mali-G715", "Mali", "Vulkan 1.3"));
    }
}
