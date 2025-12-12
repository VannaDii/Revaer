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
