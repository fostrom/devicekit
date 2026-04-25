use std::thread::sleep;
use sysinfo::{
    Components, CpuRefreshKind, DiskRefreshKind, Disks, MemoryRefreshKind, Networks, RefreshKind,
    System,
};

pub struct TelemetrySource {
    pub system: System,
    pub disks: Disks,
    pub networks: Networks,
    pub components: Components,
}

impl TelemetrySource {
    pub fn new() -> Self {
        let mut source = Self {
            system: System::new_with_specifics(
                RefreshKind::nothing()
                    .with_memory(MemoryRefreshKind::everything())
                    .with_cpu(CpuRefreshKind::everything()),
            ),
            disks: Disks::new_with_refreshed_list_specifics(DiskRefreshKind::everything()),
            networks: Networks::new_with_refreshed_list(),
            components: Components::new_with_refreshed_list(),
        };

        // warm up before returning
        source.refresh();
        sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
        source.refresh();
        source
    }

    pub fn refresh(&mut self) {
        self.system.refresh_memory();
        self.system.refresh_cpu_usage();
        self.networks.refresh(true);
        self.components.refresh(true);
        self.disks
            .refresh_specifics(true, DiskRefreshKind::everything());
    }
}
