//! Системные тамбнейлы через IShellItemImageFactory (SPEC §7.1, T6).
//!
//! GetImage → HBITMAP → GetDIBits (32-бит BGRA) → RGBA8. Типы без
//! thumbnail-handler'а получают фолбэк на системную иконку (SIIGBF_ICONONLY).

use std::path::Path;

use canvas_core::{CoreError, Thumbnail, ThumbnailProvider};
use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::{HWND, SIZE};
use windows::Win32::Graphics::Gdi::{
    DeleteObject, GetDC, GetDIBits, GetObjectW, ReleaseDC, BITMAP, BITMAPINFO, BITMAPINFOHEADER,
    BI_RGB, DIB_RGB_COLORS, HBITMAP, HGDIOBJ,
};
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};
use windows::Win32::UI::Shell::{
    IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_BIGGERSIZEOK, SIIGBF_ICONONLY,
    SIIGBF_THUMBNAILONLY,
};

/// RAII-инициализация COM на текущем потоке (worker'ы пула тамбнейлов).
struct ComGuard;

impl ComGuard {
    fn init() -> Result<Self, CoreError> {
        // SAFETY: стандартный вызов; парный CoUninitialize — в Drop. На повторном
        // вызове в том же потоке вернётся S_FALSE — это тоже Ok.
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if hr.is_ok() {
            Ok(Self)
        } else {
            Err(CoreError::Platform(format!("CoInitializeEx: {hr}")))
        }
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        // SAFETY: парность гарантирована успешным CoInitializeEx в init().
        unsafe { CoUninitialize() };
    }
}

/// RAII-обёртка HBITMAP: DeleteObject в Drop.
struct OwnedBitmap(HBITMAP);

impl Drop for OwnedBitmap {
    fn drop(&mut self) {
        // SAFETY: HBITMAP получен из IShellItemImageFactory::GetImage и не передан
        // наружу; DeleteObject идемпотентен для валидного GDI-объекта.
        unsafe {
            let _ = DeleteObject(HGDIOBJ(self.0 .0));
        }
    }
}

/// Провайдер системных тамбнейлов Windows. Stateless: COM инициализируется
/// на каждый запрос (дешёвый S_FALSE на повторе) — безопасен из любого потока.
pub struct ShellThumbnailProvider;

impl ThumbnailProvider for ShellThumbnailProvider {
    fn thumbnail(&self, path: &Path, max_size: u32) -> Result<Thumbnail, CoreError> {
        let _com = ComGuard::init()?;
        // SHCreateItemFromParsingName отклоняет (E_INVALIDARG) пути со смешанными
        // разделителями, а в .canvas слэши прямые (конвенция JSON Canvas) —
        // нормализуем в backslash
        let normalized = path.to_string_lossy().replace('/', "\\");
        let wide = HSTRING::from(normalized.as_str());
        // SAFETY: стандартный COM-вызов, `wide` живёт до конца вызова.
        let factory: IShellItemImageFactory =
            unsafe { SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()), None) }.map_err(|err| {
                CoreError::Platform(format!(
                    "SHCreateItemFromParsingName({}): {err}",
                    path.display()
                ))
            })?;
        let size = SIZE {
            cx: max_size as i32,
            cy: max_size as i32,
        };
        // Сначала — только тамбнейл; типам без thumbnail-handler'а — иконка
        // (иначе .md/.txt и т.п. остались бы совсем без картинки).
        // SAFETY: factory валиден; возвращённый HBITMAP оборачивается в OwnedBitmap.
        let bitmap = unsafe { factory.GetImage(size, SIIGBF_BIGGERSIZEOK | SIIGBF_THUMBNAILONLY) }
            .or_else(|_| unsafe { factory.GetImage(size, SIIGBF_BIGGERSIZEOK | SIIGBF_ICONONLY) })
            .map(OwnedBitmap)
            .map_err(|err| CoreError::Platform(format!("GetImage({}): {err}", path.display())))?;
        hbitmap_to_rgba(&bitmap.0)
    }
}

/// Снять пиксели HBITMAP в RGBA8 (top-down, unpremultiplied alpha).
fn hbitmap_to_rgba(hbm: &HBITMAP) -> Result<Thumbnail, CoreError> {
    let mut info = BITMAP::default();
    // SAFETY: hbm — валидный HBITMAP; info — POD размера BITMAP.
    let copied = unsafe {
        GetObjectW(
            HGDIOBJ(hbm.0),
            std::mem::size_of::<BITMAP>() as i32,
            Some(&mut info as *mut BITMAP as *mut _),
        )
    };
    if copied == 0 {
        return Err(CoreError::Platform("GetObjectW(HBITMAP) вернул 0".into()));
    }
    let (width, height) = (info.bmWidth, info.bmHeight);
    if width <= 0 || height <= 0 {
        return Err(CoreError::Platform(format!(
            "пустой битмап {width}x{height}"
        )));
    }
    let mut pixels = vec![0u8; (width * height * 4) as usize];
    let mut bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            // Отрицательная высота — top-down порядок строк
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    // SAFETY: GetDC(None) — DC всего экрана, освобождается ReleaseDC; буфер
    // pixels достаточного размера; BI_RGB без цветовой таблицы.
    let hdc = unsafe { GetDC(Some(HWND::default())) };
    let lines = unsafe {
        GetDIBits(
            hdc,
            *hbm,
            0,
            height as u32,
            Some(pixels.as_mut_ptr() as *mut _),
            &mut bmi,
            DIB_RGB_COLORS,
        )
    };
    unsafe {
        ReleaseDC(Some(HWND::default()), hdc);
    }
    if lines == 0 {
        return Err(CoreError::Platform("GetDIBits вернул 0 строк".into()));
    }
    bgra_to_rgba(&mut pixels);
    Ok(Thumbnail {
        width: width as u32,
        height: height as u32,
        rgba: pixels,
    })
}

/// BGRA → RGBA с распаковкой premultiplied alpha (shell отдаёт premultiplied).
/// Чистая функция — тестируется без COM.
pub fn bgra_to_rgba(pixels: &mut [u8]) {
    for px in pixels.chunks_exact_mut(4) {
        let (b, g, r, a) = (px[0], px[1], px[2], px[3]);
        let (r, g, b) = if a > 0 && a < 255 {
            let scale = 255.0 / f32::from(a);
            (
                (f32::from(r) * scale).min(255.0) as u8,
                (f32::from(g) * scale).min(255.0) as u8,
                (f32::from(b) * scale).min(255.0) as u8,
            )
        } else {
            (r, g, b)
        };
        px[0] = r;
        px[1] = g;
        px[2] = b;
        px[3] = a;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Swizzle: BGRA без альфы просто переставляет каналы.
    #[test]
    fn swizzle_opaque() {
        let mut px = [10, 20, 30, 255];
        bgra_to_rgba(&mut px);
        assert_eq!(px, [30, 20, 10, 255]);
    }

    /// Premultiplied alpha распаковывается: каналы делятся на a/255.
    #[test]
    fn unpremultiply_half_alpha() {
        // premultiplied: канал = исходный * 0.5
        let mut px = [32, 64, 96, 128];
        bgra_to_rgba(&mut px);
        assert_eq!(px[3], 128);
        assert!((px[0] as i32 - 192).abs() <= 1);
        assert!((px[1] as i32 - 128).abs() <= 1);
        assert!((px[2] as i32 - 64).abs() <= 1);
    }

    /// Полностью прозрачный пиксель не делится на ноль.
    #[test]
    fn transparent_is_safe() {
        let mut px = [0, 0, 0, 0];
        bgra_to_rgba(&mut px);
        assert_eq!(px, [0, 0, 0, 0]);
    }

    /// Хвост буфера, не кратный 4, не трогается и не паникует.
    #[test]
    fn tail_is_untouched() {
        let mut px = vec![10, 20, 30, 255, 7];
        bgra_to_rgba(&mut px);
        assert_eq!(px, vec![30, 20, 10, 255, 7]);
    }

    /// Интеграционные shell-тесты идут последовательно и с ретраями:
    /// IShellItemImageFactory на свежесозданном файле изредка возвращает
    /// transient-ошибку, когда тесты бегут параллельно.
    static SHELL_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn thumbnail_with_retry(path: &std::path::Path) -> Thumbnail {
        let _guard = SHELL_TEST_LOCK.lock().expect("shell test lock");
        let mut last_err = String::new();
        for attempt in 0..3 {
            match ShellThumbnailProvider.thumbnail(path, 256) {
                Ok(thumb) => return thumb,
                Err(err) => {
                    last_err = err.to_string();
                    if attempt < 2 {
                        std::thread::sleep(std::time::Duration::from_millis(200));
                    }
                }
            }
        }
        panic!("shell thumbnail не получен после 3 попыток: {last_err}");
    }

    /// Интеграционный (Windows): реальный файл → непустой RGBA8.
    /// Для .txt нет thumbnail-handler'а — срабатывает фолбэк SIIGBF_ICONONLY.
    #[test]
    fn real_file_returns_pixels() {
        let dir = std::env::temp_dir().join(format!("canvasdesk-thumb-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("tempdir");
        let file = dir.join("note.txt");
        std::fs::write(&file, b"canvasdesk").expect("write");

        let thumb = thumbnail_with_retry(&file);
        assert!(thumb.width > 0 && thumb.height > 0);
        assert_eq!(thumb.rgba.len(), (thumb.width * thumb.height * 4) as usize);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Пути из .canvas — с прямыми слэшами (конвенция JSON Canvas);
    /// SHCreateItemFromParsingName на смешанных разделителях возвращает
    /// E_INVALIDARG — провайдер обязан нормализовать.
    #[test]
    fn forward_slashes_are_accepted() {
        let dir = std::env::temp_dir().join(format!("canvasdesk-slash-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("tempdir");
        let file = dir.join("note.txt");
        std::fs::write(&file, b"canvasdesk").expect("write");
        let mixed = format!("{}/note.txt", dir.display());
        assert!(mixed.contains('/') && mixed.contains('\\'));

        thumbnail_with_retry(std::path::Path::new(&mixed));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
