use super::*;

fn fixture(count: usize) -> (tempfile::TempDir, Arc<Backend>) {
    let root = tempfile::tempdir().unwrap();
    let backend = Backend::new(root.path().join("state")).unwrap();
    let mut data = backend.lock().unwrap();
    let mut candidate = data.clone();
    candidate.photos = (0..count)
        .map(|n| Photo {
            id: n.to_string(),
            path: root
                .path()
                .join(format!("missing-{n}.jpg"))
                .to_string_lossy()
                .into(),
            name: format!("{n}.jpg"),
            width: 32,
            height: 24,
            thumbnail: "data:image/jpeg;base64,c3ludGhldGlj".into(),
        })
        .collect();
    backend.commit(&mut data, candidate).unwrap();
    drop(data);
    (root, backend)
}

#[test]
fn restores_order_cached_thumbnails_and_current_settings_without_opening_sources() {
    let (_root, backend) = fixture(5);
    let receipt = backend
        .remove_photos(vec!["1".into(), "3".into()])
        .unwrap()
        .unwrap();
    assert_eq!(receipt.count, 2);
    assert!(backend.lock().unwrap().catalog_history.undo[0]
        .photos
        .iter()
        .all(|p| p.thumbnail.capacity() == 0));
    let mut changed = RenderSettings::default();
    changed.border_percent = 17.0;
    backend.save_settings(changed).unwrap();
    let restored = backend.undo_photo_removal(receipt.id.clone()).unwrap();
    assert_eq!(
        restored
            .photos
            .iter()
            .map(|p| p.id.as_str())
            .collect::<Vec<_>>(),
        ["0", "1", "2", "3", "4"]
    );
    assert!(!restored.photos[1].thumbnail.is_empty());
    assert_eq!(backend.snapshot().unwrap().settings.border_percent, 17.0);
    assert!(backend.lock().unwrap().catalog_history.undo.is_empty());
    let redone = backend.redo_photo_removal(receipt.id.clone()).unwrap();
    assert_eq!(redone.photo_ids, ["1", "3"]);
    assert_eq!(backend.snapshot().unwrap().photos.len(), 3);
    assert_eq!(backend.snapshot().unwrap().settings.border_percent, 17.0);
    let again = backend.undo_photo_removal(receipt.id).unwrap();
    assert_eq!(
        again
            .photos
            .iter()
            .map(|p| p.id.as_str())
            .collect::<Vec<_>>(),
        ["0", "1", "2", "3", "4"]
    );
    assert!(!again.photos[1].thumbnail.is_empty());
}

#[test]
fn lifo_rejects_stale_receipts_without_consuming_history() {
    let (_root, backend) = fixture(4);
    let first = backend.remove_photos(vec!["1".into()]).unwrap().unwrap();
    let second = backend.remove_photos(vec!["2".into()]).unwrap().unwrap();
    assert!(backend.undo_photo_removal(first.id.clone()).is_err());
    backend.undo_photo_removal(second.id.clone()).unwrap();
    assert!(backend.undo_photo_removal(second.id).is_err());
    backend.undo_photo_removal(first.id).unwrap();
    assert_eq!(backend.snapshot().unwrap().photos.len(), 4);
    assert!(backend
        .remove_photos(vec!["missing".into()])
        .unwrap()
        .is_none());
}

#[test]
fn merges_reimported_paths_and_keeps_later_additions() {
    let (_root, backend) = fixture(3);
    let removed = backend.snapshot().unwrap().photos[1].clone();
    let receipt = backend
        .remove_photos(vec![removed.id.clone()])
        .unwrap()
        .unwrap();
    {
        let mut data = backend.lock().unwrap();
        let mut candidate = data.clone();
        let mut reimported = removed;
        reimported.id = "reimported".into();
        candidate.photos.push(reimported);
        let mut added = candidate.photos[0].clone();
        added.id = "added".into();
        added.path.push_str("-new");
        candidate.photos.push(added);
        backend.commit(&mut data, candidate).unwrap();
    }
    let restored = backend.undo_photo_removal(receipt.id.clone()).unwrap();
    assert_eq!(
        restored.id_map.get("1").map(String::as_str),
        Some("reimported")
    );
    assert!(backend.storage.cached_thumbnail("1").is_empty());
    assert!(!backend.storage.cached_thumbnail("reimported").is_empty());
    assert_eq!(
        restored
            .photos
            .iter()
            .map(|p| p.id.as_str())
            .collect::<Vec<_>>(),
        ["0", "reimported", "2", "added"]
    );
    let redone = backend.redo_photo_removal(receipt.id.clone()).unwrap();
    assert_eq!(redone.photo_ids, ["reimported"]);
    assert_eq!(backend.snapshot().unwrap().photos.len(), 3);
    let again = backend.undo_photo_removal(receipt.id).unwrap();
    assert_eq!(
        again
            .photos
            .iter()
            .map(|p| p.id.as_str())
            .collect::<Vec<_>>(),
        ["0", "reimported", "2", "added"]
    );
}

#[test]
fn failed_commit_keeps_catalog_and_history_for_retry() {
    let (root, backend) = fixture(3);
    let receipt = backend.remove_photos(vec!["1".into()]).unwrap().unwrap();
    let journal = root.path().join("state/state.json");
    let held = journal.with_extension("held");
    fs::rename(&journal, &held).unwrap();
    fs::create_dir(&journal).unwrap();
    assert!(backend.remove_photos(vec!["0".into()]).is_err());
    assert_eq!(backend.lock().unwrap().catalog_history.undo.len(), 1);
    assert!(backend.undo_photo_removal(receipt.id.clone()).is_err());
    assert_eq!(backend.snapshot().unwrap().photos.len(), 2);
    assert_eq!(backend.lock().unwrap().catalog_history.undo.len(), 1);
    fs::remove_dir(&journal).unwrap();
    fs::rename(held, journal).unwrap();
    backend.undo_photo_removal(receipt.id).unwrap();
    assert_eq!(backend.snapshot().unwrap().photos.len(), 3);
}

#[test]
fn bounded_history_releases_evicted_thumbnails() {
    let (_root, backend) = fixture(21);
    let mut receipts = Vec::new();
    for n in 0..21 {
        receipts.push(backend.remove_photos(vec![n.to_string()]).unwrap().unwrap());
    }
    assert_eq!(
        backend.lock().unwrap().catalog_history.undo.len(),
        REMOVAL_UNDO_LIMIT
    );
    assert!(backend.storage.cached_thumbnail("0").is_empty());
    for receipt in receipts.iter().skip(1).rev() {
        backend.undo_photo_removal(receipt.id.clone()).unwrap();
    }
    assert_eq!(
        backend
            .snapshot()
            .unwrap()
            .photos
            .iter()
            .map(|p| p.id.clone())
            .collect::<Vec<_>>(),
        (1..21).map(|n| n.to_string()).collect::<Vec<_>>()
    );
    assert!(backend.undo_photo_removal(receipts[0].id.clone()).is_err());
    assert_eq!(
        backend.lock().unwrap().catalog_history.redo.len(),
        REMOVAL_UNDO_LIMIT
    );
    for receipt in receipts.iter().skip(1) {
        backend.redo_photo_removal(receipt.id.clone()).unwrap();
    }
    assert!(backend.snapshot().unwrap().photos.is_empty());
    assert_eq!(
        backend.lock().unwrap().catalog_history.undo.len(),
        REMOVAL_UNDO_LIMIT
    );
}

#[test]
fn active_worker_blocks_catalog_undo_and_remove() {
    let (_root, backend) = fixture(2);
    let receipt = backend.remove_photos(vec!["1".into()]).unwrap().unwrap();
    backend.worker_active.store(true, Ordering::Release);
    assert!(backend.remove_photos(vec!["0".into()]).is_err());
    assert!(backend.undo_photo_removal(receipt.id.clone()).is_err());
    backend.worker_active.store(false, Ordering::Release);
    backend.undo_photo_removal(receipt.id.clone()).unwrap();
    assert_eq!(backend.snapshot().unwrap().photos.len(), 2);
    backend.worker_active.store(true, Ordering::Release);
    assert!(backend.redo_photo_removal(receipt.id.clone()).is_err());
    backend.worker_active.store(false, Ordering::Release);
    backend.redo_photo_removal(receipt.id).unwrap();
    assert_eq!(backend.snapshot().unwrap().photos.len(), 1);
}

#[test]
fn redo_replays_multiple_operations_in_order_and_rejects_duplicate_receipts() {
    let (_root, backend) = fixture(6);
    let first = backend
        .remove_photos(vec!["1".into(), "2".into(), "3".into(), "4".into()])
        .unwrap()
        .unwrap();
    let second = backend.remove_photos(vec!["5".into()]).unwrap().unwrap();
    backend.undo_photo_removal(second.id.clone()).unwrap();
    backend.undo_photo_removal(first.id.clone()).unwrap();
    assert!(backend.redo_photo_removal(second.id.clone()).is_err());
    backend.redo_photo_removal(first.id.clone()).unwrap();
    assert!(backend.redo_photo_removal(first.id.clone()).is_err());
    backend.redo_photo_removal(second.id.clone()).unwrap();
    assert_eq!(
        backend
            .snapshot()
            .unwrap()
            .photos
            .iter()
            .map(|p| p.id.as_str())
            .collect::<Vec<_>>(),
        ["0"]
    );
    backend.undo_photo_removal(second.id).unwrap();
    backend.undo_photo_removal(first.id).unwrap();
    assert_eq!(backend.snapshot().unwrap().photos.len(), 6);
    assert!(backend
        .lock()
        .unwrap()
        .catalog_history
        .redo
        .iter()
        .flat_map(|r| &r.photos)
        .all(|p| p.thumbnail.capacity() == 0));
}

#[test]
fn failed_redo_and_failed_new_branch_leave_both_stacks_available_for_retry() {
    let (root, backend) = fixture(3);
    let receipt = backend.remove_photos(vec!["1".into()]).unwrap().unwrap();
    backend.undo_photo_removal(receipt.id.clone()).unwrap();
    let journal = root.path().join("state/state.json");
    let held = journal.with_extension("held");
    fs::rename(&journal, &held).unwrap();
    fs::create_dir(&journal).unwrap();
    assert!(backend.redo_photo_removal(receipt.id.clone()).is_err());
    assert!(backend.remove_photos(vec!["0".into()]).is_err());
    assert_eq!(backend.snapshot().unwrap().photos.len(), 3);
    assert!(backend.lock().unwrap().catalog_history.undo.is_empty());
    assert_eq!(backend.lock().unwrap().catalog_history.redo.len(), 1);
    fs::remove_dir(&journal).unwrap();
    fs::rename(held, journal).unwrap();
    backend.redo_photo_removal(receipt.id.clone()).unwrap();
    backend.undo_photo_removal(receipt.id.clone()).unwrap();
    assert!(backend
        .remove_photos(vec!["missing".into()])
        .unwrap()
        .is_none());
    assert_eq!(backend.lock().unwrap().catalog_history.redo.len(), 1);
    backend.remove_photos(vec!["0".into()]).unwrap();
    assert!(backend.redo_photo_removal(receipt.id).is_err());
    assert!(backend.lock().unwrap().catalog_history.redo.is_empty());
}

#[test]
fn new_import_invalidates_redo_but_noop_or_failed_import_does_not() {
    let (root, backend) = fixture(2);
    let receipt = backend.remove_photos(vec!["1".into()]).unwrap().unwrap();
    backend.undo_photo_removal(receipt.id.clone()).unwrap();
    backend
        .import_photos(vec![root
            .path()
            .join("not-a-photo.txt")
            .to_string_lossy()
            .into()])
        .unwrap();
    assert_eq!(backend.lock().unwrap().catalog_history.redo.len(), 1);
    let source = root.path().join("new.png");
    image::RgbImage::new(16, 12).save(&source).unwrap();
    let journal = root.path().join("state/state.json");
    let held = journal.with_extension("held");
    fs::rename(&journal, &held).unwrap();
    fs::create_dir(&journal).unwrap();
    assert!(backend
        .import_photos(vec![source.to_string_lossy().into()])
        .is_err());
    assert_eq!(backend.lock().unwrap().catalog_history.redo.len(), 1);
    fs::remove_dir(&journal).unwrap();
    fs::rename(held, journal).unwrap();
    assert_eq!(
        backend
            .import_photos(vec![source.to_string_lossy().into()])
            .unwrap()
            .photos
            .len(),
        1
    );
    assert!(backend.lock().unwrap().catalog_history.redo.is_empty());
    assert!(backend.redo_photo_removal(receipt.id).is_err());
}

fn selection(active: &str, ids: &[&str], anchor: &str) -> PhotoSelection {
    PhotoSelection {
        active_id: active.into(),
        ids: ids.iter().map(|id| id.to_string()).collect(),
        anchor_id: anchor.into(),
    }
}

fn restart(root: &tempfile::TempDir, backend: Arc<Backend>) -> Arc<Backend> {
    drop(backend);
    Backend::new(root.path().join("state")).unwrap()
}

#[test]
fn restart_restores_undo_redo_order_selection_and_anchor_without_sources() {
    let (root, backend) = fixture(6);
    let range = selection("4", &["1", "2", "3", "4"], "1");
    let receipt = backend
        .remove_photos_with_selection(
            vec!["1".into(), "2".into(), "3".into(), "4".into()],
            Some(range.clone()),
        )
        .unwrap()
        .unwrap();
    let backend = restart(&root, backend);
    let snapshot = backend.snapshot().unwrap();
    assert_eq!(snapshot.catalog_history.undo.len(), 1);
    assert_eq!(snapshot.catalog_history.undo[0].selection, range);
    assert_eq!(snapshot.photo_selection, selection("5", &["5"], "5"));
    let restored = backend.undo_photo_removal(receipt.id.clone()).unwrap();
    assert_eq!(restored.photo_selection, range);
    assert_eq!(
        restored
            .photos
            .iter()
            .map(|p| p.id.as_str())
            .collect::<Vec<_>>(),
        ["0", "1", "2", "3", "4", "5"]
    );
    let backend = restart(&root, backend);
    assert_eq!(backend.snapshot().unwrap().photo_selection, range);
    assert_eq!(backend.snapshot().unwrap().catalog_history.redo.len(), 1);
    let changed = selection("0", &[], "0");
    backend
        .save_photo_selection(
            changed.clone(),
            backend.snapshot().unwrap().catalog_revision,
        )
        .unwrap();
    let backend = restart(&root, backend);
    assert_eq!(backend.snapshot().unwrap().photo_selection, changed);
    backend
        .redo_photo_removal_with_selection(receipt.id.clone(), Some(changed.clone()))
        .unwrap();
    let backend = restart(&root, backend);
    assert_eq!(backend.snapshot().unwrap().photos.len(), 2);
    let restored = backend.undo_photo_removal(receipt.id).unwrap();
    assert_eq!(restored.photo_selection, changed);
    assert!(restored.photos.iter().all(|p| !p.thumbnail.is_empty()));
    assert!(!fs::read_to_string(root.path().join("state/state.json"))
        .unwrap()
        .contains("base64,"));
}

#[test]
fn restart_keeps_both_stacks_at_once_and_persists_new_branches() {
    let (root, backend) = fixture(4);
    let first = backend
        .remove_photos_with_selection(vec!["1".into()], Some(selection("1", &["1"], "1")))
        .unwrap()
        .unwrap();
    let second = backend
        .remove_photos_with_selection(vec!["2".into()], Some(selection("2", &["2"], "2")))
        .unwrap()
        .unwrap();
    backend.undo_photo_removal(second.id.clone()).unwrap();
    let backend = restart(&root, backend);
    let snapshot = backend.snapshot().unwrap();
    assert_eq!(snapshot.catalog_history.undo[0].id, first.id);
    assert_eq!(snapshot.catalog_history.redo[0].id, second.id);
    backend.redo_photo_removal(second.id.clone()).unwrap();
    let backend = restart(&root, backend);
    backend.undo_photo_removal(second.id.clone()).unwrap();
    backend.undo_photo_removal(first.id.clone()).unwrap();
    let backend = restart(&root, backend);
    backend.redo_photo_removal(first.id).unwrap();
    backend.remove_photos(vec!["3".into()]).unwrap();
    let backend = restart(&root, backend);
    assert!(backend.snapshot().unwrap().catalog_history.redo.is_empty());
    assert!(backend.redo_photo_removal(second.id).is_err());
}

#[test]
fn twenty_operation_limit_survives_repeated_restarts() {
    let (root, mut backend) = fixture(21);
    let mut receipts = Vec::new();
    for n in 0..21 {
        receipts.push(backend.remove_photos(vec![n.to_string()]).unwrap().unwrap());
        backend = restart(&root, backend);
    }
    assert_eq!(backend.snapshot().unwrap().catalog_history.undo.len(), 20);
    assert!(backend.undo_photo_removal(receipts[0].id.clone()).is_err());
    for receipt in receipts.iter().skip(1).rev() {
        backend.undo_photo_removal(receipt.id.clone()).unwrap();
        backend = restart(&root, backend);
    }
    let snapshot = backend.snapshot().unwrap();
    assert_eq!(snapshot.photos.len(), 20);
    assert_eq!(snapshot.catalog_history.redo.len(), 20);
    for receipt in receipts.iter().skip(1) {
        backend.redo_photo_removal(receipt.id.clone()).unwrap();
        backend = restart(&root, backend);
    }
    assert!(backend.snapshot().unwrap().photos.is_empty());
    assert_eq!(backend.snapshot().unwrap().catalog_history.undo.len(), 20);
}

#[test]
fn selection_sidecar_is_small_atomic_and_rejects_old_catalog_revisions() {
    let (root, backend) = fixture(4);
    let journal = root.path().join("state/state.json");
    let unchanged = fs::read(&journal).unwrap();
    let selected = selection("3", &["1", "2", "3"], "1");
    assert!(backend.save_photo_selection(selected.clone(), 0).unwrap());
    assert_eq!(fs::read(&journal).unwrap(), unchanged);
    assert!(
        fs::metadata(root.path().join("state/catalog-selection.json"))
            .unwrap()
            .len()
            < 512
    );
    let backend = restart(&root, backend);
    assert_eq!(backend.snapshot().unwrap().photo_selection, selected);
    let receipt = backend
        .remove_photos_with_selection(vec!["2".into()], Some(selected))
        .unwrap()
        .unwrap();
    assert!(!backend
        .save_photo_selection(selection("0", &["0"], "0"), 0)
        .unwrap());
    let expected = backend.snapshot().unwrap().photo_selection;
    let backend = restart(&root, backend);
    assert_eq!(backend.snapshot().unwrap().photo_selection, expected);
    let selection_file = root.path().join("state/catalog-selection.json");
    fs::remove_file(&selection_file).unwrap();
    fs::create_dir(&selection_file).unwrap();
    assert!(backend
        .save_photo_selection(selection("0", &["0"], "0"), receipt.catalog_revision)
        .is_err());
    assert_eq!(backend.snapshot().unwrap().photo_selection, expected);
    assert_eq!(backend.snapshot().unwrap().catalog_history.undo.len(), 1);
}

#[test]
fn legacy_state_and_damaged_history_preserve_the_catalog() {
    let (root, backend) = fixture(3);
    drop(backend);
    let journal = root.path().join("state/state.json");
    let mut value: serde_json::Value =
        serde_json::from_slice(&fs::read(&journal).unwrap()).unwrap();
    for key in ["catalogHistory", "catalogRevision", "photoSelection"] {
        value.as_object_mut().unwrap().remove(key);
    }
    fs::write(&journal, serde_json::to_vec(&value).unwrap()).unwrap();
    let backend = Backend::new(root.path().join("state")).unwrap();
    assert_eq!(backend.snapshot().unwrap().photos.len(), 3);
    assert!(backend.snapshot().unwrap().catalog_history.undo.is_empty());
    drop(backend);
    value["catalogHistory"] = serde_json::json!({"undo":"broken", "redo":[]});
    fs::write(&journal, serde_json::to_vec(&value).unwrap()).unwrap();
    let backend = Backend::new(root.path().join("state")).unwrap();
    let snapshot = backend.snapshot().unwrap();
    assert_eq!(snapshot.photos.len(), 3);
    assert!(snapshot.catalog_history.undo.is_empty());
    assert!(snapshot.warnings.iter().any(|s| s.contains("历史记录")));
}

#[test]
fn journal_backup_recovers_catalog_and_history_together() {
    let (root, backend) = fixture(4);
    let before = selection("2", &["1", "2"], "1");
    let receipt = backend
        .remove_photos_with_selection(vec!["1".into(), "2".into()], Some(before.clone()))
        .unwrap()
        .unwrap();
    backend.save_settings(RenderSettings::default()).unwrap(); // Backup now contains the same removal.
    drop(backend);
    fs::write(root.path().join("state/state.json"), b"{broken").unwrap();
    let backend = Backend::new(root.path().join("state")).unwrap();
    assert_eq!(backend.snapshot().unwrap().photos.len(), 2);
    assert_eq!(backend.snapshot().unwrap().catalog_history.undo.len(), 1);
    assert_eq!(
        backend
            .undo_photo_removal(receipt.id)
            .unwrap()
            .photo_selection,
        before
    );
}
