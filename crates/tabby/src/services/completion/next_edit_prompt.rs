//! Построение промптов для предсказания следующих правок кода
//!
//! Модуль предоставляет функциональность для создания специализированных промптов
//! на основе истории редактирования кода. Используется для обучения и инференса
//! моделей, предсказывающих следующие изменения в коде.

// === IMPORTS ===
use super::EditHistory;

// === STRUCTS ===
/// Построитель промптов для предсказания следующих правок
///
/// Создаёт структурированные промпты с разметкой для обучения моделей
/// предсказанию следующих изменений в коде на основе истории редактирования.
/// Использует специальные теги для разделения различных версий кода.
///
/// # Examples
///
/// ```rust
/// let builder = NextEditPromptBuilder::new();
/// let prompt = builder.build_prompt(&edit_history);
/// ```
pub struct NextEditPromptBuilder;

// === IMPLEMENTATIONS ===
impl NextEditPromptBuilder {
    /// Создаёт новый построитель промптов
    ///
    /// Возвращает экземпляр построителя, готовый для создания промптов
    /// на основе истории редактирования кода.
    pub fn new() -> Self {
        Self
    }

    /// Строит промпт для предсказания следующих правок
    ///
    /// Создаёт структурированный промпт с разметкой, содержащий:
    /// - Оригинальный код в теге `<|original_code|>`
    /// - Diff изменений в теге `<|edits_diff|>`
    /// - Текущую версию кода в теге `<|current_version|>`
    /// - Заготовку для следующей версии в теге `<|next_version|>`
    ///
    /// # Arguments
    ///
    /// * `edit_history` - История редактирования с оригинальным кодом,
    ///   diff'ом изменений и текущей версией
    ///
    /// # Returns
    ///
    /// Строка с форматированным промптом, готовым для подачи в модель
    ///
    /// # Examples
    ///
    /// ```rust
    /// let edit_history = EditHistory {
    ///     original_code: "fn hello() {}".to_string(),
    ///     edits_diff: "+    println!(\"Hello!\");".to_string(),
    ///     current_version: "fn hello() {\n    println!(\"Hello!\");\n}".to_string(),
    /// };
    /// 
    /// let builder = NextEditPromptBuilder::new();
    /// let prompt = builder.build_prompt(&edit_history);
    /// ```
    pub fn build_prompt(&self, edit_history: &EditHistory) -> String {
        format!(
            "<|original_code|>\n{}\n<|edits_diff|>\n{}\n<|current_version|>\n{}\n<|next_version|>\n",
            edit_history.original_code,
            edit_history.edits_diff,
            edit_history.current_version
        )
    }
}

// === IMPLEMENTATIONS ===
impl Default for NextEditPromptBuilder {
    /// Создаёт построитель промптов по умолчанию
    ///
    /// Эквивалентно вызову `NextEditPromptBuilder::new()`.
    fn default() -> Self {
        Self::new()
    }
}

// === TESTS ===
#[cfg(test)]
mod tests {
    use super::*;

    /// Создаёт тестовую историю редактирования
    fn create_test_edit_history() -> EditHistory {
        EditHistory {
            original_code: "fn main() {\n    println!(\"Hello, world!\");\n}".to_string(),
            edits_diff: "---src/main.rs\n+++src/main.rs\n@@ -1,1 +1,2 @@\n    println!(\"Hello, world!\");\n    let x = 5;\n    println!(\"Hello, world!\");".to_string(),
            current_version: "fn main() {\n    let x = 5;\n    println!(\"Hello, world!\");\n}".to_string(),
        }
    }

    #[test]
    fn test_build_prompt_contains_required_tags() {
        let edit_history = create_test_edit_history();
        let builder = NextEditPromptBuilder::new();
        let prompt = builder.build_prompt(&edit_history);

        // Проверяем наличие всех обязательных тегов разметки
        assert!(prompt.contains("<|original_code|>"), "Промпт должен содержать тег original_code");
        assert!(prompt.contains("<|edits_diff|>"), "Промпт должен содержать тег edits_diff");
        assert!(prompt.contains("<|current_version|>"), "Промпт должен содержать тег current_version");
        assert!(prompt.contains("<|next_version|>"), "Промпт должен содержать тег next_version");
    }

    #[test]
    fn test_build_prompt_contains_code_content() {
        let edit_history = create_test_edit_history();
        let builder = NextEditPromptBuilder::new();
        let prompt = builder.build_prompt(&edit_history);

        // Проверяем наличие содержимого кода в промпте
        assert!(prompt.contains("fn main()"), "Промпт должен содержать функцию main");
        assert!(prompt.contains("let x = 5;"), "Промпт должен содержать объявление переменной");
        assert!(prompt.contains("println!"), "Промпт должен содержать вызов println!");
    }

    #[test]
    fn test_build_prompt_structure() {
        let edit_history = create_test_edit_history();
        let builder = NextEditPromptBuilder::new();
        let prompt = builder.build_prompt(&edit_history);

        // Проверяем правильную последовательность тегов
        let original_pos = prompt.find("<|original_code|>").unwrap();
        let diff_pos = prompt.find("<|edits_diff|>").unwrap();
        let current_pos = prompt.find("<|current_version|>").unwrap();
        let next_pos = prompt.find("<|next_version|>").unwrap();

        assert!(original_pos < diff_pos, "original_code должен быть перед edits_diff");
        assert!(diff_pos < current_pos, "edits_diff должен быть перед current_version");
        assert!(current_pos < next_pos, "current_version должен быть перед next_version");
    }

    #[test]
    fn test_new_and_default_equivalence() {
        let builder1 = NextEditPromptBuilder::new();
        let builder2 = NextEditPromptBuilder::default();
        
        let edit_history = create_test_edit_history();
        let prompt1 = builder1.build_prompt(&edit_history);
        let prompt2 = builder2.build_prompt(&edit_history);

        assert_eq!(prompt1, prompt2, "new() и default() должны создавать идентичные построители");
    }

    #[test]
    fn test_build_prompt_with_empty_content() {
        let edit_history = EditHistory {
            original_code: "".to_string(),
            edits_diff: "".to_string(),
            current_version: "".to_string(),
        };

        let builder = NextEditPromptBuilder::new();
        let prompt = builder.build_prompt(&edit_history);

        // Даже с пустым содержимым теги должны присутствовать
        assert!(prompt.contains("<|original_code|>"));
        assert!(prompt.contains("<|edits_diff|>"));
        assert!(prompt.contains("<|current_version|>"));
        assert!(prompt.contains("<|next_version|>"));
    }
}
