use super::*;

#[test]
fn fs_column_mappings_are_stable() {
    assert_eq!(FsStringField::LibraryRoot.column(), "library_root");
    assert_eq!(FsStringField::Par2.column(), "par2");
    assert_eq!(FsStringField::MoveMode.column(), "move_mode");
    assert_eq!(FsBooleanField::Extract.column(), "extract");
    assert_eq!(FsBooleanField::Flatten.column(), "flatten");
    assert_eq!(FsArrayField::CleanupKeep.column(), "cleanup_keep");
    assert_eq!(FsArrayField::CleanupDrop.column(), "cleanup_drop");
    assert_eq!(FsArrayField::AllowPaths.column(), "allow_paths");
    assert_eq!(FsOptionalStringField::ChmodFile.column(), "chmod_file");
    assert_eq!(FsOptionalStringField::ChmodDir.column(), "chmod_dir");
    assert_eq!(FsOptionalStringField::Owner.column(), "owner");
    assert_eq!(FsOptionalStringField::Group.column(), "group");
    assert_eq!(FsOptionalStringField::Umask.column(), "umask");
}

#[test]
fn queue_policy_set_flags_round_trip() {
    let set = QueuePolicySet::from_flags([true, false, true]);
    assert!(set.auto_managed());
    assert!(!set.prefer_seeds());
    assert!(set.dont_count_slow());
}

#[test]
fn seeding_toggle_set_flags_round_trip() {
    let set = SeedingToggleSet::from_flags([true, true, false]);
    assert!(set.sequential_default());
    assert!(set.super_seeding());
    assert!(!set.strict_super_seeding());
}

#[test]
fn storage_toggle_set_flags_round_trip() {
    let set = StorageToggleSet::from_flags([true, false, true, false]);
    assert!(set.use_partfile());
    assert!(!set.coalesce_reads());
    assert!(set.coalesce_writes());
    assert!(!set.use_disk_cache_pool());
}

#[test]
fn nat_toggle_set_flags_round_trip() {
    let set = NatToggleSet::from_flags([true, false, true, false]);
    assert!(set.lsd());
    assert!(!set.upnp());
    assert!(set.natpmp());
    assert!(!set.pex());
}

#[test]
fn privacy_toggle_set_flags_round_trip() {
    let set = PrivacyToggleSet::from_flags([true, true, false, false, true, false]);
    assert!(set.anonymous_mode());
    assert!(set.force_proxy());
    assert!(!set.prefer_rc4());
    assert!(!set.allow_multiple_connections_per_ip());
    assert!(set.enable_outgoing_utp());
    assert!(!set.enable_incoming_utp());
}
