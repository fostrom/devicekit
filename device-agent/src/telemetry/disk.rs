use serde::Serialize;
use std::{cmp::Ordering, collections::HashMap};
use sysinfo::Disks;

#[derive(Debug, Clone, Serialize)]
pub struct Disk {
    pub mount_point: String,
    pub name: String,
    pub filesystem: String,
    pub read_only: bool,
    pub available_bytes: u64,
    pub total_bytes: u64,
    pub bytes_read: u64,
    pub bytes_written: u64,
}

pub fn fetch_disks(disks: &Disks) -> Vec<Disk> {
    let collected = disks
        .list()
        .iter()
        .map(make_disk)
        .filter(filter_disk)
        .collect::<Vec<_>>();

    let mut cleaned = eliminate_duplicates(collected);
    cleaned.sort_by(|a, b| a.mount_point.cmp(&b.mount_point).then(a.name.cmp(&b.name)));
    cleaned
}

pub fn diff_disks(old: &Vec<Disk>, new: &mut Vec<Disk>) -> Vec<Disk> {
    [].to_vec()
}

fn make_disk(disk: &sysinfo::Disk) -> Disk {
    let usage = disk.usage();

    Disk {
        mount_point: disk.mount_point().to_string_lossy().to_string(),
        name: disk.name().to_string_lossy().to_string(),
        filesystem: disk.file_system().to_string_lossy().to_ascii_lowercase(),
        read_only: disk.is_read_only(),
        available_bytes: disk.available_space(),
        total_bytes: disk.total_space(),
        bytes_read: usage.read_bytes,
        bytes_written: usage.written_bytes,
    }
}

fn filter_disk(disk: &Disk) -> bool {
    if disk.total_bytes == 0 {
        return false;
    }

    if disk.filesystem.is_empty() {
        return false;
    }

    !matches!(
        disk.filesystem.as_str(),
        "autofs"
            | "binfmt_misc"
            | "cgroup"
            | "cgroup2"
            | "configfs"
            | "debugfs"
            | "devfs"
            | "devtmpfs"
            | "fdescfs"
            | "fusectl"
            | "hugetlbfs"
            | "kernfs"
            | "linprocfs"
            | "linsysfs"
            | "mqueue"
            | "nfs"
            | "nfs4"
            | "nsfs"
            | "nullfs"
            | "overlay"
            | "proc"
            | "procfs"
            | "pstore"
            | "ramfs"
            | "rpc_pipefs"
            | "securityfs"
            | "smbfs"
            | "squashfs"
            | "sysfs"
            | "tmpfs"
            | "tracefs"
            | "virtiofs"
    )
}

fn eliminate_duplicates(disks: Vec<Disk>) -> Vec<Disk> {
    let mut selected = HashMap::<(String, String, u64), Disk>::new();

    for disk in disks {
        let key = (disk.name.clone(), disk.filesystem.clone(), disk.total_bytes);

        selected
            .entry(key)
            .and_modify(|existing| {
                if rank_disks(&disk, existing) == Ordering::Less {
                    *existing = disk.clone();
                }
            })
            .or_insert(disk);
    }

    selected.into_values().collect::<Vec<_>>()
}

fn rank_disks(disk_a: &Disk, disk_b: &Disk) -> Ordering {
    mount_rank(disk_a)
        .cmp(&mount_rank(disk_b))
        .then_with(|| disk_a.mount_point.len().cmp(&disk_b.mount_point.len()))
        .then_with(|| disk_a.mount_point.cmp(&disk_b.mount_point))
}

fn mount_rank(disk: &Disk) -> u8 {
    if disk.mount_point == "/" {
        0
    } else if !disk.read_only {
        1
    } else {
        2
    }
}
