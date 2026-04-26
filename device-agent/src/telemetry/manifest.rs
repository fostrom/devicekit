use super::source::TelemetrySource;
use serde::Serialize;
use serde_json::Value;
use std::env::var;
use sysinfo::{Product, System};

#[derive(Debug, Clone, Serialize)]
pub struct Manifest {
    pub agent_version: String,
    pub distribution_id: String,
    pub cpu_arch: String,
    pub cpu_logical_cores: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub long_os_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kernel_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_physical_cores: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_brand: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_vendor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_family: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_vendor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sdk: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sdk_manifest: Option<Value>,
}

pub fn collect_manifest(source: &TelemetrySource) -> Manifest {
    let (sdk, sdk_manifest) = read_sdk_manifest();

    Manifest {
        agent_version: env!("CARGO_PKG_VERSION").to_string(),
        os_name: System::name(),
        hostname: System::host_name(),
        distribution_id: System::distribution_id(),
        os_version: System::os_version(),
        long_os_version: System::long_os_version(),
        kernel_version: System::kernel_version(),
        cpu_arch: System::cpu_arch(),
        cpu_logical_cores: source.system.cpus().len(),
        cpu_physical_cores: System::physical_core_count(),
        cpu_brand: first_non_empty(source.system.cpus().iter().map(|cpu| cpu.brand())),
        cpu_vendor: first_non_empty(source.system.cpus().iter().map(|cpu| cpu.vendor_id())),
        product_name: Product::name().filter(|value| !value.is_empty()),
        product_family: Product::family().filter(|value| !value.is_empty()),
        product_version: Product::version().filter(|value| !value.is_empty()),
        product_vendor: Product::vendor_name().filter(|value| !value.is_empty()),
        sdk,
        sdk_manifest,
    }
}

fn read_sdk_manifest() -> (Option<String>, Option<Value>) {
    let Ok(raw) = var("FOSTROM_SDK_MANIFEST") else {
        return (None, None);
    };

    if raw.trim().is_empty() {
        return (None, None);
    }

    let parsed: Value = match serde_json::from_str(&raw) {
        Ok(value) => value,
        Err(e) => {
            eprintln!("manifest: failed to parse FOSTROM_SDK_MANIFEST: {e}");
            return (None, None);
        }
    };

    let Value::Object(mut map) = parsed else {
        eprintln!("manifest: FOSTROM_SDK_MANIFEST must be a JSON object");
        return (None, None);
    };

    let sdk = match map.remove("sdk") {
        Some(Value::String(s)) if is_known_sdk(&s) => s,
        Some(Value::String(s)) => {
            eprintln!("manifest: FOSTROM_SDK_MANIFEST has unknown sdk {s:?}");
            return (None, None);
        }
        _ => {
            eprintln!("manifest: FOSTROM_SDK_MANIFEST is missing the sdk field");
            return (None, None);
        }
    };

    let sdk_manifest = match map.remove("sdk_manifest") {
        Some(value @ Value::Object(_)) => value,
        _ => {
            eprintln!("manifest: FOSTROM_SDK_MANIFEST is missing the sdk_manifest object");
            return (None, None);
        }
    };

    (Some(sdk), Some(sdk_manifest))
}

fn is_known_sdk(value: &str) -> bool {
    matches!(value, "elixir" | "python" | "js")
}

fn first_non_empty<'a>(values: impl Iterator<Item = &'a str>) -> Option<String> {
    values
        .map(str::trim)
        .find(|value| !value.is_empty())
        .map(ToString::to_string)
}
