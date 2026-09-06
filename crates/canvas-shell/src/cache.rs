//! Тамбнейл-кэш в SQLite (SPEC §5.2, T6): метаданные в `~/.canvasdesk/cache.db`,
//! RGBA-блобы — файлами `~/.canvasdesk/thumbs/<hash>.rgba`. Инвалидация по mtime.
//!
//! Кэш пересоздаваемый (SPEC §5.3): его удаление ничего не ломает, а любая
//! ошибка БД/блоба трактуется как промах, а не как сбой приложения.

use std::fs;
use std::path::{Path, PathBuf};

use canvas_core::{CoreError, Thumbnail};

/// Класс размера тамбнейла (длинная сторона) — единственный на M1 (SPEC §6.4).
pub const SIZE_CLASS: u32 = 256;

/// FNV-1a 64-бит: дешёвый хэш содержимого блоба без внешних зависимостей.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// mtime файла в unix-секундах; None — файл недоступен (битая ссылка).
pub fn file_mtime_secs(path: &Path) -> Option<i64> {
    let modified = fs::metadata(path).ok()?.modified().ok()?;
    let secs = modified
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    Some(secs as i64)
}

/// Каталог кэша по умолчанию: `~/.canvasdesk` (SPEC §5.2).
pub fn default_cache_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(|home| PathBuf::from(home).join(".canvasdesk"))
}

/// Тамбнейл-кэш: таблица thumb_cache + блобы файлами.
pub struct ThumbCache {
    conn: rusqlite::Connection,
    blobs_dir: PathBuf,
}

impl ThumbCache {
    /// Открыть/создать кэш в каталоге `dir` (будет создан вместе с thumbs/).
    pub fn open(dir: &Path) -> Result<Self, CoreError> {
        let blobs_dir = dir.join("thumbs");
        fs::create_dir_all(&blobs_dir)?;
        let conn = rusqlite::Connection::open(dir.join("cache.db"))
            .map_err(|err| CoreError::Platform(format!("SQLite open: {err}")))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS thumb_cache(
                file_path TEXT NOT NULL,
                mtime INTEGER NOT NULL,
                size_class INTEGER NOT NULL,
                width INTEGER NOT NULL,
                height INTEGER NOT NULL,
                blob_hash TEXT NOT NULL,
                PRIMARY KEY(file_path, size_class)
            );",
        )
        .map_err(|err| CoreError::Platform(format!("SQLite schema: {err}")))?;
        Ok(Self { conn, blobs_dir })
    }

    /// Тамбнейл из кэша; промах — если записи нет, mtime изменился или блоб утерян.
    pub fn get(&self, path: &Path, mtime: i64, size_class: u32) -> Option<Thumbnail> {
        let key = path.to_string_lossy();
        let row = self
            .conn
            .query_row(
                "SELECT mtime, width, height, blob_hash FROM thumb_cache
                 WHERE file_path = ?1 AND size_class = ?2",
                rusqlite::params![key.as_ref(), size_class],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, u32>(1)?,
                        row.get::<_, u32>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
            .ok()?;
        let (stored_mtime, width, height, blob_hash) = row;
        if stored_mtime != mtime {
            return None;
        }
        let rgba = fs::read(self.blobs_dir.join(format!("{blob_hash}.rgba"))).ok()?;
        if rgba.len() != (width * height * 4) as usize {
            return None;
        }
        Some(Thumbnail {
            width,
            height,
            rgba,
        })
    }

    /// Записать тамбнейл в кэш (INSERT OR REPLACE — переиспользуется и для mtime-апдейта).
    pub fn put(
        &self,
        path: &Path,
        mtime: i64,
        size_class: u32,
        thumb: &Thumbnail,
    ) -> Result<(), CoreError> {
        let blob_hash = format!("{:016x}", fnv1a(&thumb.rgba));
        let blob_path = self.blobs_dir.join(format!("{blob_hash}.rgba"));
        if !blob_path.exists() {
            fs::write(&blob_path, &thumb.rgba)?;
        }
        let key = path.to_string_lossy();
        self.conn
            .execute(
                "INSERT OR REPLACE INTO thumb_cache
                 (file_path, mtime, size_class, width, height, blob_hash)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![
                    key.as_ref(),
                    mtime,
                    size_class,
                    thumb.width,
                    thumb.height,
                    blob_hash
                ],
            )
            .map_err(|err| CoreError::Platform(format!("SQLite put: {err}")))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Временный каталог без внешних зависимостей (pid + атомарный счётчик).
    fn temp_dir(tag: &str) -> PathBuf {
        static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "canvasdesk-test-{}-{}-{tag}",
            std::process::id(),
            COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).expect("tempdir");
        dir
    }

    fn sample_thumb() -> Thumbnail {
        Thumbnail {
            width: 2,
            height: 2,
            rgba: vec![255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 9, 9, 9, 255],
        }
    }

    /// Round-trip: put → get возвращает те же пиксели.
    #[test]
    fn cache_round_trip() {
        let dir = temp_dir("roundtrip");
        let cache = ThumbCache::open(&dir).expect("open");
        let thumb = sample_thumb();
        cache
            .put(Path::new("C:/a.png"), 42, SIZE_CLASS, &thumb)
            .expect("put");
        let got = cache
            .get(Path::new("C:/a.png"), 42, SIZE_CLASS)
            .expect("hit");
        assert_eq!(got, thumb);
        let _ = fs::remove_dir_all(&dir);
    }

    /// Инвалидация по mtime (SPEC §5.2): другой mtime — промах.
    #[test]
    fn invalidated_by_mtime() {
        let dir = temp_dir("mtime");
        let cache = ThumbCache::open(&dir).expect("open");
        cache
            .put(Path::new("C:/a.png"), 42, SIZE_CLASS, &sample_thumb())
            .expect("put");
        assert!(cache.get(Path::new("C:/a.png"), 43, SIZE_CLASS).is_none());
        // Другой size_class — тоже промах
        assert!(cache.get(Path::new("C:/a.png"), 42, 128).is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    /// Утерянный блоб-файл — промах, а не ошибка (SPEC §5.3).
    #[test]
    fn missing_blob_is_miss() {
        let dir = temp_dir("blob");
        let cache = ThumbCache::open(&dir).expect("open");
        cache
            .put(Path::new("C:/a.png"), 42, SIZE_CLASS, &sample_thumb())
            .expect("put");
        let _ = fs::remove_dir_all(dir.join("thumbs"));
        assert!(cache.get(Path::new("C:/a.png"), 42, SIZE_CLASS).is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    /// Повторный put по тому же ключу перезаписывает запись.
    #[test]
    fn put_replaces() {
        let dir = temp_dir("replace");
        let cache = ThumbCache::open(&dir).expect("open");
        cache
            .put(Path::new("C:/a.png"), 1, SIZE_CLASS, &sample_thumb())
            .expect("put 1");
        let mut newer = sample_thumb();
        newer.rgba[0] = 7;
        cache
            .put(Path::new("C:/a.png"), 2, SIZE_CLASS, &newer)
            .expect("put 2");
        assert_eq!(cache.get(Path::new("C:/a.png"), 2, SIZE_CLASS), Some(newer));
        let _ = fs::remove_dir_all(&dir);
    }

    /// mtime реального файла читается; несуществующего — None.
    #[test]
    fn mtime_of_real_file() {
        let dir = temp_dir("mtime-fs");
        let file = dir.join("f.txt");
        fs::write(&file, b"x").expect("write");
        assert!(file_mtime_secs(&file).is_some());
        assert!(file_mtime_secs(&dir.join("nope")).is_none());
        let _ = fs::remove_dir_all(&dir);
    }
}
