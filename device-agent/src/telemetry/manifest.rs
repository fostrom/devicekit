use super::source::TelemetrySource;
use serde::Serialize;
use sysinfo::{Product, System};

#[derive(Debug, Clone, Serialize)]
pub struct Manifest {
    pub agent_version: String,
    pub os_name: Option<String>,
    pub hostname: Option<String>,
    pub distribution_id: String,
    pub os_version: Option<String>,
    pub long_os_version: Option<String>,
    pub kernel_version: Option<String>,
    pub cpu_arch: String,
    pub cpu_logical_cores: usize,
    pub cpu_physical_cores: Option<usize>,
    pub cpu_brand: Option<String>,
    pub cpu_vendor: Option<String>,
    pub product_name: Option<String>,
    pub product_family: Option<String>,
    pub product_version: Option<String>,
    pub product_vendor: Option<String>,
}

pub fn collect_manifest(source: &TelemetrySource) -> Manifest {
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
    }
}

fn first_non_empty<'a>(values: impl Iterator<Item = &'a str>) -> Option<String> {
    values
        .map(str::trim)
        .find(|value| !value.is_empty())
        .map(ToString::to_string)
}
