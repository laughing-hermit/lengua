use lengua_idea::lock::Sealed;
use wasmtime::Engine;
use wasmtime::component::Component;

#[allow(unsafe_code)]
pub fn load(engine: &Engine, sealed: &Sealed) -> Result<Component, String> {
    let loaded = unsafe { Component::deserialize(engine, sealed.bytes()) };
    loaded.map_err(|e| {
        format!(
            "wasmtime could not load the sealed code: {e:#}; run `check` to seal it again on this computer"
        )
    })
}
