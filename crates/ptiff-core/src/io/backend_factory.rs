//! Process-wide registry mapping a backend name to a factory function.
//!
//! Mirrors `ptiff::io::BackendFactory` (see
//! `libptiff/include/ptiff/io/backend_factory.hpp`). It is the **one deliberate
//! singleton** in this codebase: each concrete backend can self-register at
//! first use, and callers look up a backend by name to turn it into a
//! [`crate::io::StorageBackend`]. The registry is internally synchronized, so
//! registration and lookup are thread-safe.

use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};

use crate::io::StorageBackend;
use crate::{Error, Result};

/// Backend builder: produces a fresh [`StorageBackend`] on each call.
///
/// Wraps a closure so a backend can register anything that owns whatever it
/// needs to construct, mirroring the C++ `Builder = std::function<...>`.
pub type Builder = Box<dyn Fn() -> Box<dyn StorageBackend> + Send + Sync>;

/// The sole process-wide [`BackendFactory`].
pub struct BackendFactory {
    /// Name -> builder registry, synchronized for thread-safe static use.
    registry: Mutex<BTreeMap<String, Builder>>,
}

impl BackendFactory {
    /// Builds an empty factory (only useful for tests / composition).
    pub fn new() -> Self {
        Self {
            registry: Mutex::new(BTreeMap::new()),
        }
    }

    /// Returns the process-wide factory, registering built-in backends on first
    /// use (mirrors the C++ static-initializer self-registration).
    pub fn instance() -> &'static BackendFactory {
        static INSTANCE: OnceLock<BackendFactory> = OnceLock::new();
        INSTANCE.get_or_init(|| {
            let factory = BackendFactory::new();
            factory.register_defaults();
            factory
        })
    }

    /// Registers the built-in backends that this build was compiled with.
    fn register_defaults(&self) {
        #[cfg(feature = "memory-backend")]
        self.register(
            "memory",
            Box::new(|| Box::new(crate::io::backend::MemoryBackend)),
        )
        .expect("built-in backend names must not collide");
    }

    /// Registers a `builder` under `name`.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorCode::InvalidArgument`] if `name` is already registered
    /// (registration is idempotency-checked and errors on a clash).
    pub fn register(&self, name: &str, builder: Builder) -> Result<()> {
        let mut registry = self
            .registry
            .lock()
            .map_err(|_| Error::unknown("BackendFactory: registry lock poisoned"))?;
        if registry.contains_key(name) {
            return Err(Error::invalid_argument(
                "BackendFactory::registerBackend: name already registered",
            ));
        }
        registry.insert(name.to_string(), builder);
        Ok(())
    }

    /// Creates a backend by `name`.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorCode::NotFound`] if `name` was never registered.
    pub fn create(&self, name: &str) -> Result<Box<dyn StorageBackend>> {
        let registry = self
            .registry
            .lock()
            .map_err(|_| Error::unknown("BackendFactory: registry lock poisoned"))?;
        let builder = registry
            .get(name)
            .ok_or_else(|| Error::not_found("BackendFactory::create: no such backend"))?;
        Ok((builder)())
    }

    /// Returns the names of all currently registered backends (order
    /// unspecified).
    pub fn registered_backends(&self) -> Vec<String> {
        let registry = self
            .registry
            .lock()
            .map_err(|_| Error::unknown("BackendFactory: registry lock poisoned"))
            .expect("registry lock");
        registry.keys().cloned().collect()
    }
}

impl Default for BackendFactory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::BackendCapabilities;
    use crate::io::StorageModel;
    use crate::ErrorCode;

    /// A trivial backend used to probe the registry.
    #[derive(Default)]
    struct Probe;

    impl StorageBackend for Probe {
        fn name(&self) -> &'static str {
            "probe"
        }
        fn capabilities(&self) -> BackendCapabilities {
            BackendCapabilities::default()
        }
        fn open_image_source<'a>(
            &self,
            _reader: &'a mut dyn crate::io::BinaryReader,
        ) -> Result<Box<dyn crate::io::ImageSource + 'a>> {
            Err(Error::not_implemented("probe"))
        }
        fn open_image_sink<'a>(
            &self,
            _writer: &'a mut dyn crate::io::BinaryWriter,
            _model: &StorageModel,
        ) -> Result<Box<dyn crate::io::ImageSink + 'a>> {
            Err(Error::not_implemented("probe"))
        }
        fn deserialize_model(
            &self,
            _reader: &mut dyn crate::io::BinaryReader,
        ) -> Result<StorageModel> {
            Err(Error::not_implemented("probe"))
        }
        fn serialize_model(
            &self,
            _model: &StorageModel,
            _writer: &mut dyn crate::io::BinaryWriter,
        ) -> Result<()> {
            Err(Error::not_implemented("probe"))
        }
    }

    #[test]
    fn create_unknown_backend_is_not_found() {
        let factory = BackendFactory::new();
        let err = match factory.create("no-such-backend") {
            Ok(_) => panic!("expected error"),
            Err(e) => e,
        };
        assert_eq!(err.code(), ErrorCode::NotFound);
        assert_eq!(err.message(), "BackendFactory::create: no such backend");
    }

    #[test]
    fn register_then_create_round_trips() {
        let factory = BackendFactory::new();
        factory
            .register("probe", Box::new(|| Box::new(Probe)))
            .expect("register");
        let backend = match factory.create("probe") {
            Ok(b) => b,
            Err(e) => panic!("create failed: {e}"),
        };
        assert_eq!(backend.name(), "probe");
    }

    #[test]
    fn duplicate_registration_is_invalid_argument() {
        let factory = BackendFactory::new();
        factory
            .register("probe", Box::new(|| Box::new(Probe)))
            .expect("first");
        let err = factory
            .register("probe", Box::new(|| Box::new(Probe)))
            .unwrap_err();
        assert_eq!(err.code(), ErrorCode::InvalidArgument);
        assert_eq!(
            err.message(),
            "BackendFactory::registerBackend: name already registered"
        );
    }

    #[test]
    fn registered_backends_lists_names() {
        let factory = BackendFactory::new();
        factory
            .register("alpha", Box::new(|| Box::new(Probe)))
            .expect("alpha");
        factory
            .register("beta", Box::new(|| Box::new(Probe)))
            .expect("beta");
        let mut names = factory.registered_backends();
        names.sort();
        assert_eq!(names, vec!["alpha", "beta"]);
    }

    #[test]
    fn each_create_builds_a_fresh_backend() {
        let factory = BackendFactory::new();
        factory
            .register("probe", Box::new(|| Box::new(Probe)))
            .expect("register");
        let a = match factory.create("probe") {
            Ok(b) => b,
            Err(e) => panic!("create failed: {e}"),
        };
        let b = match factory.create("probe") {
            Ok(v) => v,
            Err(e) => panic!("create failed: {e}"),
        };
        assert_eq!(a.name(), "probe");
        assert_eq!(b.name(), "probe");
    }

    #[cfg(feature = "memory-backend")]
    #[test]
    fn singleton_self_registers_memory_backend() {
        let factory = BackendFactory::instance();
        let backend = match factory.create("memory") {
            Ok(b) => b,
            Err(e) => panic!("memory backend should self-register: {e}"),
        };
        assert_eq!(backend.name(), "memory");
    }
}
