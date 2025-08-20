// === IMPORTS ===
use dashmap::DashMap;
use tabby_common::languages::Language;
use trie_rs::{Trie, TrieBuilder};

// === TYPE_ALIASES ===
/// Ссылка на кэшированное префиксное дерево из DashMap
type CachedTrie<'a> = dashmap::mapref::one::Ref<'a, String, Trie<u8>>;

// === STRUCTS ===
/// Фабрика для создания условий остановки декодирования
///
/// Управляет кэшированием префиксных деревьев (Trie) для эффективного
/// определения стоп-слов при генерации кода. Поддерживает как языковые
/// стоп-слова, так и дополнительные стоп-слова из конфигурации модели.
///
/// # Кэширование
///
/// Префиксные деревья кэшируются по языку для избежания повторного построения
/// при множественных запросах для одного языка программирования.
pub struct StopConditionFactory {
    /// Кэш префиксных деревьев по языкам программирования
    stop_trie_cache: DashMap<String, Trie<u8>>,
    /// Дополнительные стоп-слова из конфигурации модели
    stop_words_from_model_config: Vec<String>,
}

/// Условие остановки для процесса декодирования
///
/// Отслеживает процесс генерации текста и определяет, когда следует
/// остановить декодирование на основе найденных стоп-слов. Использует
/// обращенный текст для эффективного поиска суффиксов.
///
/// # Алгоритм
///
/// 1. Текст обращается для поиска суффиксов как префиксов
/// 2. При каждом новом токене текст обновляется
/// 3. Выполняется поиск по префиксному дереву
/// 4. Возвращается решение об остановке и длина совпадения
pub struct StopCondition<'a> {
    /// Префиксное дерево для поиска стоп-слов (опционально)
    stop_trie: Option<CachedTrie<'a>>,
    /// Обращенный текст для эффективного поиска суффиксов
    reversed_text: String,
    /// Количество декодированных токенов
    num_decoded: usize,
}

// === IMPLEMENTATIONS ===
impl Default for StopConditionFactory {
    fn default() -> Self {
        Self {
            stop_trie_cache: DashMap::new(),
            stop_words_from_model_config: vec![],
        }
    }
}

impl StopConditionFactory {
    /// Создает фабрику с дополнительными стоп-словами
    ///
    /// # Arguments
    ///
    /// * `stop_words` - Дополнительные стоп-слова из конфигурации модели
    ///
    /// # Examples
    ///
    /// ```rust
    /// let factory = StopConditionFactory::with_stop_words(vec![
    ///     "<|endoftext|>".to_string(),
    ///     "<|file_sep|>".to_string(),
    /// ]);
    /// ```
    pub fn with_stop_words(stop_words: Vec<String>) -> Self {
        Self {
            stop_trie_cache: DashMap::new(),
            stop_words_from_model_config: stop_words,
        }
    }

    /// Создает условие остановки для заданного текста и языка
    ///
    /// # Arguments
    ///
    /// * `text` - Начальный текст для анализа
    /// * `language` - Язык программирования (опционально)
    ///
    /// # Returns
    ///
    /// Возвращает настроенное условие остановки
    pub fn create<'a>(&'a self, text: &'a str, language: Option<&'static Language>) -> StopCondition<'a> {
        if let Some(language) = language {
            StopCondition::new(self.get_trie(language), text)
        } else {
            StopCondition::new(None, text)
        }
    }

    /// Получает или создает префиксное дерево для языка
    ///
    /// Объединяет стоп-слова языка с дополнительными стоп-словами модели
    /// и кэширует результирующее дерево для повторного использования.
    fn get_trie<'a>(&'a self, language: &'static Language) -> Option<CachedTrie<'a>> {
        let mut stop_words = language.get_stop_words();
        // Добавляем стоп-слова модели к языковым стоп-словам
        stop_words.extend(self.stop_words_from_model_config.iter().cloned());

        if stop_words.is_empty() {
            None
        } else {
            let hashkey = language.language().to_owned();
            let mut trie = self.stop_trie_cache.get(&hashkey);
            if trie.is_none() {
                self.stop_trie_cache
                    .insert(hashkey.clone(), create_stop_trie(stop_words));
                trie = self.stop_trie_cache.get(&hashkey);
            }

            trie
        }
    }
}

impl<'a> StopCondition<'a> {
    /// Создает новое условие остановки
    ///
    /// # Arguments
    ///
    /// * `stop_trie` - Префиксное дерево для поиска стоп-слов
    /// * `text` - Начальный текст
    pub fn new(stop_trie: Option<CachedTrie<'a>>, text: &str) -> Self {
        Self {
            stop_trie,
            reversed_text: reverse(text),
            num_decoded: 0,
        }
    }

    /// Проверяет, следует ли остановить декодирование
    ///
    /// Обновляет внутреннее состояние новым текстом и проверяет
    /// наличие стоп-слов в конце текущего содержимого.
    ///
    /// # Arguments
    ///
    /// * `new_text` - Новый декодированный текст
    ///
    /// # Returns
    ///
    /// Кортеж (следует_остановить, длина_совпадения)
    pub fn should_stop(&mut self, new_text: &str) -> (bool, usize) {
        self.num_decoded += 1;
        if !new_text.is_empty() {
            // Добавляем новый обращенный текст к началу (так как текст уже обращен)
            self.reversed_text = reverse(new_text) + &self.reversed_text;

            if let Some(re) = &self.stop_trie {
                let matches = re.common_prefix_search(&self.reversed_text);
                let matched_length = matches.into_iter().map(|x| x.len()).max();
                if let Some(matched_length) = matched_length {
                    return (true, matched_length);
                }
            }
        }
        (false, 0)
    }
}

// === FREE_FUNCTIONS ===
/// Обращает строку по символам
///
/// Используется для преобразования текста в формат, подходящий
/// для поиска суффиксов как префиксов в Trie структуре.
///
/// # Arguments
///
/// * `s` - Строка для обращения
///
/// # Returns
///
/// Обращенная строка
fn reverse<T>(s: T) -> String
where
    T: Into<String>,
{
    s.into().chars().rev().collect()
}

/// Создает префиксное дерево из списка стоп-слов
///
/// Все стоп-слова обращаются перед добавлением в дерево,
/// что позволяет эффективно искать суффиксы в тексте.
///
/// # Arguments
///
/// * `stop_words` - Список стоп-слов для добавления в дерево
///
/// # Returns
///
/// Построенное префиксное дерево
fn create_stop_trie(stop_words: Vec<String>) -> Trie<u8> {
    let mut builder = TrieBuilder::new();
    for word in stop_words {
        // Обращаем каждое стоп-слово для поиска суффиксов
        builder.push(reverse(word))
    }
    builder.build()
}

// === TESTS ===
#[cfg(test)]
mod tests {
    use tabby_common::languages::UNKNOWN_LANGUAGE;

    use super::*;

    #[test]
    fn test_trie_works() {
        let text = reverse("void write_u32(std::uint32_t val) const {\n        write_raw(&val, sizeof(val));\n    }\n\n    ~llama_file() {\n        if (fp) {\n            std::fclose(fp);\n        }\n    }\n};\n\nvoid");

        // Тест с простыми стоп-словами - не должно найти совпадений
        let trie = create_stop_trie(vec!["\n\n".to_owned(), "\n\n  ".to_owned()]);
        assert!(trie.common_prefix_search(&text).is_empty());

        // Тест с расширенным набором стоп-слов - должно найти совпадения
        let trie = create_stop_trie(vec![
            "\n\n".to_owned(),
            "\n\n  ".to_owned(),
            "\nvoid".to_owned(),
            "<|file_sep|>".to_owned(), // стиль qwen 2.5 coder
        ]);
        assert!(!trie.common_prefix_search(&text).is_empty());

        // Тест специфичного стоп-слова qwen25coder
        let qwen25coder = reverse("qwen25 style stop words;<|file_sep|>");
        assert!(!trie.common_prefix_search(qwen25coder).is_empty());
    }

    #[test]
    fn test_stop_condition_max_length() {
        let factory = StopConditionFactory::default();
        let mut cond = factory.create("", Some(&UNKNOWN_LANGUAGE));
        
        // Тестируем, что обычные символы не вызывают остановку
        let (should_stop, _) = cond.should_stop("1");
        assert!(!should_stop);
        let (should_stop, _) = cond.should_stop("2");
        assert!(!should_stop);
        let (should_stop, _) = cond.should_stop("3");
        assert!(!should_stop);
        let (should_stop, _) = cond.should_stop("4");
        assert!(!should_stop)
    }

    #[test]
    fn test_stop_condition_additional_stop_words() {
        let factory = StopConditionFactory::with_stop_words(vec!["<|endoftext|>".to_owned()]);
        let mut cond = factory.create("", Some(&UNKNOWN_LANGUAGE));
        
        // Обычный символ не должен вызывать остановку
        let (should_stop, _) = cond.should_stop("1");
        assert!(!should_stop);
        
        // Стоп-слово должно вызывать остановку
        let (should_stop, _) = cond.should_stop("<|endoftext|>");
        assert!(should_stop);
    }
}
