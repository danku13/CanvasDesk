//! FR-076: экспорт самодостаточного HTML («артефакт защиты») — команда
//! нативного приложения. Идея перенесена из Archify (практика «portable
//! by default», карта переноса T1): асинхронная защита расчёта — стейкхолдер
//! получает один офлайн-файл вместо живого окна (GAP-01).
//!
//! Сборка артефакта — чистая функция ядра (`canvas_core::export_html`);
//! здесь — только платформенная проводка: снимок базового пересчёта
//! (double buffer `flow_baseline`, FR-064 P1), тема из настроек и запись
//! файла рядом с канвасом (`<имя>.canvas` → `<имя>.html`).

use super::*;

impl App {
    /// FR-076 (Ctrl+Shift+E): собрать HTML-артефакт и записать рядом с
    /// канвасом. Значения — базовый пересчёт (what-if подмены НЕ входят:
    /// артефакт отражает модель, а не активную сессию what-if — та же
    /// семантика, что у `export_active` веб-версии: экспортируется
    /// сохраняемое состояние). Таблица сравнения — из сценариев канваса.
    pub(super) fn export_html_artifact(&mut self) {
        // Снимок решений: read-гард двойного буфера; export_html — чистая
        // функция, borrow-область гарда кончается до мутации (show_toast)
        let solutions = canvas_scene::read_flow(&self.scene.flow_baseline);
        let comparison = canvas_core::export_html::scenario_comparison_for_export(
            &self.scene.canvas,
            &solutions,
        );
        let title = self
            .scene
            .path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .map(str::to_owned)
            .unwrap_or_else(|| "CanvasDesk".to_owned());
        let options = canvas_core::export_html::ExportHtmlOptions {
            title,
            dark: matches!(self.settings.theme, canvas_core::Theme::Dark),
        };
        let html = canvas_core::export_html::export_html(
            &self.scene.canvas,
            &solutions,
            comparison.as_ref(),
            &options,
        );
        drop(solutions);
        let target = self.scene.path.with_extension("html");
        match std::fs::write(&target, &html) {
            Ok(()) => {
                tracing::info!(
                    path = %target.display(),
                    bytes = html.len(),
                    "FR-076: HTML-артефакт записан"
                );
                self.show_toast(self.trf(
                    keys::TOAST_EXPORT_HTML_OK,
                    &[("path", &target.display().to_string())],
                ));
            }
            Err(err) => {
                tracing::error!(
                    path = %target.display(),
                    %err,
                    "FR-076: запись HTML-артефакта не удалась"
                );
                self.show_toast(
                    self.trf(keys::TOAST_EXPORT_HTML_FAIL, &[("err", &err.to_string())]),
                );
            }
        }
    }
}
