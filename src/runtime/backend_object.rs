use std::{any::Any, sync::Arc};

#[derive(Clone)]
pub(crate) struct BackendObject(Arc<dyn Any + Send + Sync>);

impl BackendObject {
    pub(crate) fn new<T>(value: T) -> Self
    where
        T: Any + Send + Sync,
    {
        Self(Arc::new(value))
    }

    pub(crate) fn downcast_ref<T>(&self) -> Option<&T>
    where
        T: Any,
    {
        self.0.as_ref().downcast_ref::<T>()
    }
}
