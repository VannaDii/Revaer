use super::*;

#[test]
fn engine_column_mappings_are_stable() {
    assert_eq!(EngineBooleanField::Dht.column(), "dht");
    assert_eq!(
        EngineBooleanField::SequentialDefault.column(),
        "sequential_default"
    );
    assert_eq!(EngineTextField::ResumeDir.column(), "resume_dir");
    assert_eq!(EngineTextField::DownloadRoot.column(), "download_root");
    assert_eq!(EngineRateField::MaxDownloadBps.column(), "max_download_bps");
    assert_eq!(EngineRateField::MaxUploadBps.column(), "max_upload_bps");
}

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
