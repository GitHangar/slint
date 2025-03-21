// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint.dev/logo/slint-logo-square-light.svg")]

extern crate alloc;

use event_loop::CustomEvent;
use i_slint_core::platform::EventLoopProxy;
use i_slint_core::window::WindowAdapter;
use renderer::WinitCompatibleRenderer;
use std::rc::Rc;

mod winitwindowadapter;
use i_slint_core::platform::PlatformError;
use winitwindowadapter::*;
pub(crate) mod event_loop;

/// Re-export of the winit crate.
pub use winit;

/// Internal type used by the winit backend for thread communication and window system updates.
#[non_exhaustive]
#[derive(Debug)]
pub enum SlintUserEvent {
    CustomEvent { event: CustomEvent },
}

mod renderer {
    use i_slint_core::platform::PlatformError;

    pub(crate) trait WinitCompatibleRenderer {
        fn new(
            window_builder: winit::window::WindowBuilder,
        ) -> Result<(Self, winit::window::Window), PlatformError>
        where
            Self: Sized;

        fn render(&self, window: &i_slint_core::api::Window) -> Result<(), PlatformError>;

        fn as_core_renderer(&self) -> &dyn i_slint_core::renderer::Renderer;

        // Got WindowEvent::Occluded
        fn occluded(&self, _: bool) {}

        // Got winit::Event::Resumed
        fn resumed(&self, _winit_window: &winit::window::Window) -> Result<(), PlatformError> {
            Ok(())
        }
    }

    #[cfg(feature = "renderer-femtovg")]
    pub(crate) mod femtovg;
    #[cfg(enable_skia_renderer)]
    pub(crate) mod skia;

    #[cfg(feature = "renderer-software")]
    pub(crate) mod sw;
}

#[cfg(enable_accesskit)]
mod accesskit;

#[cfg(target_arch = "wasm32")]
pub(crate) mod wasm_input_helper;

#[cfg(target_arch = "wasm32")]
pub fn create_gl_window_with_canvas_id(
    canvas_id: &str,
) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
    WinitWindowAdapter::new::<crate::renderer::femtovg::GlutinFemtoVGRenderer>(canvas_id)
}

fn window_factory_fn<R: WinitCompatibleRenderer + 'static>(
) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
    WinitWindowAdapter::new::<R>(
        #[cfg(target_arch = "wasm32")]
        "canvas".into(),
    )
}

cfg_if::cfg_if! {
    if #[cfg(feature = "renderer-femtovg")] {
        type DefaultRenderer = renderer::femtovg::GlutinFemtoVGRenderer;
        const DEFAULT_RENDERER_NAME: &str = "FemtoVG";
    } else if #[cfg(enable_skia_renderer)] {
        type DefaultRenderer = renderer::skia::WinitSkiaRenderer;
        const DEFAULT_RENDERER_NAME: &'static str = "Skia";
    } else if #[cfg(feature = "renderer-software")] {
        type DefaultRenderer = renderer::sw::WinitSoftwareRenderer;
        const DEFAULT_RENDERER_NAME: &'static str = "Software";
    } else {
        compile_error!("Please select a feature to build with the winit backend: `renderer-femtovg`, `renderer-skia`, `renderer-skia-opengl`, `renderer-skia-vulkan` or `renderer-software`");
    }
}

fn try_create_window_with_fallback_renderer() -> Option<Rc<dyn WindowAdapter>> {
    #[cfg(any(
        feature = "renderer-skia",
        feature = "renderer-skia-opengl",
        feature = "renderer-skia-vulkan"
    ))]
    {
        if let Ok(window) = window_factory_fn::<renderer::skia::WinitSkiaRenderer>() {
            return Some(window);
        }
    }
    #[cfg(any(feature = "renderer-femtovg"))]
    {
        if let Ok(window) = window_factory_fn::<renderer::femtovg::GlutinFemtoVGRenderer>() {
            return Some(window);
        }
    }
    #[cfg(any(feature = "renderer-software"))]
    {
        if let Ok(window) = window_factory_fn::<renderer::sw::WinitSoftwareRenderer>() {
            return Some(window);
        }
    }

    None
}

#[doc(hidden)]
pub type NativeWidgets = ();
#[doc(hidden)]
pub type NativeGlobals = ();
#[doc(hidden)]
pub const HAS_NATIVE_STYLE: bool = false;
#[doc(hidden)]
pub mod native_widgets {}

#[doc = concat!("This struct implements the Slint Platform trait. Use this in conjuction with [`slint::platform::set_platform`](https://slint.dev/releases/", env!("CARGO_PKG_VERSION"), "/docs/rust/slint/platform/fn.set_platform.html) to initialize.")]
/// Slint to use winit for all windowing system interaction.
///
/// ```rust,no_run
/// # use i_slint_backend_winit::Backend;
/// slint::platform::set_platform(Box::new(Backend::new()));
/// ```
pub struct Backend {
    window_factory_fn: fn() -> Result<Rc<dyn WindowAdapter>, PlatformError>,
}

impl Default for Backend {
    fn default() -> Self {
        Self::new()
    }
}

impl Backend {
    #[doc = concat!("Creates a new winit backend with the default renderer that's compiled in. See the [backend documentation](https://slint.dev/releases/", env!("CARGO_PKG_VERSION"), "/docs/rust/slint/index.html#backends) for")]
    /// details on how to select the default renderer.
    pub fn new() -> Self {
        Self::new_with_renderer_by_name(None)
    }

    #[doc = concat!("Creates a new winit backend with the renderer specified by name. See the [backend documentation](https://slint.dev/releases/", env!("CARGO_PKG_VERSION"), "/docs/rust/slint/index.html#backends) for")]
    /// details on how to select the default renderer.
    /// If the renderer name is `None` or the name is not recognized, the default renderer is selected.
    pub fn new_with_renderer_by_name(renderer_name: Option<&str>) -> Self {
        let window_factory_fn = match renderer_name {
            #[cfg(feature = "renderer-femtovg")]
            Some("gl") | Some("femtovg") => {
                window_factory_fn::<renderer::femtovg::GlutinFemtoVGRenderer>
            }
            #[cfg(enable_skia_renderer)]
            Some("skia") => window_factory_fn::<renderer::skia::WinitSkiaRenderer>,
            #[cfg(feature = "renderer-software")]
            Some("sw") | Some("software") => {
                window_factory_fn::<renderer::sw::WinitSoftwareRenderer>
            }
            None => window_factory_fn::<DefaultRenderer>,
            Some(renderer_name) => {
                eprintln!(
                    "slint winit: unrecognized renderer {}, falling back to {}",
                    renderer_name, DEFAULT_RENDERER_NAME
                );
                window_factory_fn::<DefaultRenderer>
            }
        };
        Self { window_factory_fn }
    }
}

fn send_event_via_global_event_loop_proxy(
    event: SlintUserEvent,
) -> Result<(), i_slint_core::api::EventLoopError> {
    #[cfg(not(target_arch = "wasm32"))]
    crate::event_loop::GLOBAL_PROXY
        .get_or_init(Default::default)
        .lock()
        .unwrap()
        .send_event(event)?;
    #[cfg(target_arch = "wasm32")]
    {
        crate::event_loop::GLOBAL_PROXY.with(|global_proxy| {
            let mut maybe_proxy = global_proxy.borrow_mut();
            let proxy = maybe_proxy.get_or_insert_with(Default::default);
            // Calling send_event is usually done by winit at the bottom of the stack,
            // in event handlers, and thus winit might decide to process the event
            // immediately within that stack.
            // To prevent re-entrancy issues that might happen by getting the application
            // event processed on top of the current stack, set winit in Poll mode so that
            // events are queued and process on top of a clean stack during a requested animation
            // frame a few moments later.
            // This also allows batching multiple post_event calls and redraw their state changes
            // all at once.
            proxy.send_event(SlintUserEvent::CustomEvent {
                event: CustomEvent::WakeEventLoopWorkaround,
            })?;
            proxy.send_event(event)?;
            Ok(())
        })?
    }
    Ok(())
}

impl i_slint_core::platform::Platform for Backend {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        (self.window_factory_fn)().or_else(|e| {
            try_create_window_with_fallback_renderer().ok_or_else(|| {
                format!("Winit backend failed to find a suitable renderer. Last failure was: {e}")
                    .into()
            })
        })
    }

    #[doc(hidden)]
    fn set_event_loop_quit_on_last_window_closed(&self, quit_on_last_window_closed: bool) {
        event_loop::QUIT_ON_LAST_WINDOW_CLOSED
            .store(quit_on_last_window_closed, std::sync::atomic::Ordering::Relaxed);
    }

    fn run_event_loop(&self) -> Result<(), PlatformError> {
        crate::event_loop::run()
    }

    fn new_event_loop_proxy(&self) -> Option<Box<dyn EventLoopProxy>> {
        struct Proxy;
        impl EventLoopProxy for Proxy {
            fn quit_event_loop(&self) -> Result<(), i_slint_core::api::EventLoopError> {
                send_event_via_global_event_loop_proxy(SlintUserEvent::CustomEvent {
                    event: CustomEvent::Exit,
                })
            }

            fn invoke_from_event_loop(
                &self,
                event: Box<dyn FnOnce() + Send>,
            ) -> Result<(), i_slint_core::api::EventLoopError> {
                let e = SlintUserEvent::CustomEvent { event: CustomEvent::UserEvent(event) };
                send_event_via_global_event_loop_proxy(e)
            }
        }
        Some(Box::new(Proxy))
    }

    fn set_clipboard_text(&self, text: &str, clipboard: i_slint_core::platform::Clipboard) {
        crate::event_loop::with_window_target(|event_loop_target| {
            event_loop_target.clipboard(clipboard)?.set_contents(text.into()).ok()
        });
    }

    fn clipboard_text(&self, clipboard: i_slint_core::platform::Clipboard) -> Option<String> {
        crate::event_loop::with_window_target(|event_loop_target| {
            event_loop_target.clipboard(clipboard)?.get_contents().ok()
        })
    }
}

/// Invokes the specified callback with a reference to the [`winit::event_loop::EventLoopWindowTarget`].
/// Use this to get access to the display connection or create new windows with [`winit::window::WindowBuilder`].
///
/// *Note*: This function can only be called from within the Slint main thread.
pub fn with_event_loop_window_target<R>(
    callback: impl FnOnce(&winit::event_loop::EventLoopWindowTarget<SlintUserEvent>) -> R,
) -> R {
    crate::event_loop::with_window_target(|event_loop_interface| {
        callback(event_loop_interface.event_loop_target())
    })
}

mod private {
    pub trait WinitWindowAccessorSealed {}
}

#[doc = concat!("This helper trait can be used to obtain access to the [`winit::window::Window`] for a given [`slint::Window`](https://slint.dev/releases/", env!("CARGO_PKG_VERSION"), "/docs/rust/slint/struct.window).")]
pub trait WinitWindowAccessor: private::WinitWindowAccessorSealed {
    /// Returns true if a [`winit::window::Window`] exists for this window. This is the case if the window is
    /// backed by this winit backend.
    fn has_winit_window(&self) -> bool;
    /// Invokes the specified callback with a reference to the [`winit::window::Window`] that exists for this Slint window
    /// and returns `Some(T)`; otherwise `None`.
    fn with_winit_window<T>(&self, callback: impl FnOnce(&winit::window::Window) -> T)
        -> Option<T>;
}

impl WinitWindowAccessor for i_slint_core::api::Window {
    fn has_winit_window(&self) -> bool {
        winit_window_rc_for_window(self).is_some()
    }

    fn with_winit_window<T>(
        &self,
        callback: impl FnOnce(&winit::window::Window) -> T,
    ) -> Option<T> {
        winit_window_rc_for_window(self).as_ref().map(|w| callback(w))
    }
}

impl private::WinitWindowAccessorSealed for i_slint_core::api::Window {}

fn winit_window_rc_for_window(
    window: &i_slint_core::api::Window,
) -> Option<Rc<winit::window::Window>> {
    i_slint_core::window::WindowInner::from_pub(window)
        .window_adapter()
        .internal(i_slint_core::InternalToken)
        .and_then(|wa| wa.as_any().downcast_ref::<WinitWindowAdapter>())
        .map(|adapter| adapter.winit_window())
}

#[cfg(test)]
mod testui {
    slint::slint! {
        export component App inherits Window {
            Text { text: "Ok"; }
        }
    }
}

// Sorry, can't test with rust test harness and multiple threads.
#[cfg(not(any(target_arch = "wasm32", target_os = "macos", target_os = "ios")))]
#[test]
fn test_window_accessor() {
    slint::platform::set_platform(Box::new(crate::Backend::new())).unwrap();

    use testui::*;
    let app = App::new().unwrap();
    let slint_window = app.window();
    assert!(slint_window.has_winit_window());
}
