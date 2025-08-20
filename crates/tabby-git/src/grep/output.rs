// === IMPORTS ===
use std::{
    io::BufRead,
    path::{Path, PathBuf},
};

use grep::{
    matcher::Matcher, 
    regex::RegexMatcher, 
    searcher::Sink
};

use tracing::debug;

use super::{
    GrepFile, 
    GrepLine, 
    GrepSubMatch, 
    GrepTextOrBase64
};

// === STRUCTS ===
/// Обработчик вывода операций grep с поддержкой различных типов поиска
///
/// Собирает результаты поиска в файле и управляет их отправкой через канал.
/// Поддерживает как позитивный поиск (поиск совпадений), так и негативный
/// (исключение файлов с совпадениями). Отслеживает состояние поиска на
/// уровне файла и содержимого для принятия решений о фильтрации.
///
/// # Поля
///
/// * `path` - Путь к обрабатываемому файлу
/// * `lines` - Собранные строки с совпадениями
/// * `tx` - Канал для отправки результатов
/// * `content_matched` - Найдены ли совпадения в содержимом
/// * `content_negated` - Исключен ли файл по содержимому
/// * `file_matched` - Соответствует ли файл критериям поиска
/// * `file_negated` - Исключен ли файл по имени/пути
///
/// # Examples
///
/// ```rust
/// let (tx, rx) = tokio::sync::mpsc::channel(100);
/// let mut output = GrepOutput::new(PathBuf::from("src/main.rs"), tx);
/// 
/// // Использование с matcher'ом
/// let mut sink = output.sink(&matcher);
/// searcher.search_path(&matcher, &path, sink)?;
/// ```
pub struct GrepOutput {
    path: PathBuf,
    lines: Vec<GrepLine>,

    tx: tokio::sync::mpsc::Sender<GrepFile>,

    content_matched: bool,
    content_negated: bool,

    pub file_matched: bool,
    pub file_negated: bool,
}

/// Sink для обработки позитивных совпадений в grep поиске
///
/// Реализует трейт `Sink` из библиотеки grep для сбора совпадений
/// и контекстных строк. Автоматически создает `GrepSubMatch` объекты
/// для всех найденных совпадений в строке.
///
/// # Lifetime Parameters
///
/// * `'processor` - Время жизни ссылки на GrepOutput
/// * `'pattern` - Время жизни ссылки на RegexMatcher
pub struct GrepMatchSink<'processor, 'pattern> {
    output: &'processor mut GrepOutput,
    matcher: &'pattern RegexMatcher,
}

/// Sink для обработки негативных совпадений в grep поиске
///
/// Используется для исключения файлов, содержащих нежелательные паттерны.
/// При первом совпадении помечает файл как исключенный и прекращает поиск.
///
/// # Lifetime Parameters
///
/// * `'processor` - Время жизни ссылки на GrepOutput
pub struct GrepNegativeMatchSink<'processor> {
    output: &'processor mut GrepOutput,
}

// === IMPLEMENTATIONS ===
impl std::fmt::Debug for GrepOutput {
    /// Форматирование для отладки с отображением состояния поиска
    ///
    /// Показывает только флаги состояния без чувствительных данных
    /// как пути файлов или содержимое строк для безопасной отладки.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GrepOutput")
            .field("content_matched", &self.content_matched)
            .field("content_negated", &self.content_negated)
            .field("file_matched", &self.file_matched)
            .field("file_negated", &self.file_negated)
            .finish()
    }
}

impl GrepOutput {
    /// Создает новый обработчик вывода для указанного файла
    ///
    /// Инициализирует все флаги состояния в false и подготавливает
    /// пустой вектор для сбора строк с совпадениями.
    ///
    /// # Arguments
    ///
    /// * `path` - Путь к файлу для обработки
    /// * `tx` - Канал для отправки результатов поиска
    ///
    /// # Returns
    ///
    /// Новый экземпляр GrepOutput готовый к использованию
    pub fn new(path: PathBuf, tx: tokio::sync::mpsc::Sender<GrepFile>) -> Self {
        Self {
            path: path.to_owned(),
            lines: Vec::new(),
            tx,

            file_matched: false,
            file_negated: false,

            content_matched: false,
            content_negated: false,
        }
    }

    /// Возвращает путь к обрабатываемому файлу
    ///
    /// # Returns
    ///
    /// Ссылка на путь файла
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Создает sink для обработки позитивных совпадений
    ///
    /// Возвращает объект, реализующий трейт `Sink` для использования
    /// с библиотекой grep. Автоматически собирает все совпадения
    /// и контекстные строки в процессе поиска.
    ///
    /// # Arguments
    ///
    /// * `matcher` - Regex matcher для поиска совпадений в строках
    ///
    /// # Returns
    ///
    /// GrepMatchSink готовый к использованию с searcher'ом
    pub fn sink<'processor, 'pattern>(
        &'processor mut self,
        matcher: &'pattern RegexMatcher,
    ) -> GrepMatchSink<'processor, 'pattern> {
        GrepMatchSink {
            output: self,
            matcher,
        }
    }

    /// Создает sink для обработки негативных совпадений
    ///
    /// Используется для исключения файлов, содержащих определенные паттерны.
    /// При первом совпадении файл помечается как исключенный.
    ///
    /// # Returns
    ///
    /// GrepNegativeMatchSink готовый к использованию
    pub fn negative_sink<'processor>(&'processor mut self) -> GrepNegativeMatchSink<'processor> {
        GrepNegativeMatchSink { output: self }
    }

    /// Записывает строку с совпадением в коллекцию результатов
    ///
    /// Внутренний метод для добавления обработанных строк в вектор.
    /// Используется sink'ами для сохранения найденных совпадений.
    ///
    /// # Arguments
    ///
    /// * `line` - Обработанная строка с метаданными и совпадениями
    fn record(&mut self, line: GrepLine) {
        self.lines.push(line);
    }

    /// Завершает обработку и отправляет результаты через канал
    ///
    /// Применяет фильтры на основе требований к совпадениям файла и содержимого.
    /// Если совпадения в содержимом не найдены, читает первые 5 строк файла
    /// для предоставления контекста. Игнорирует ошибки отправки, так как
    /// они могут возникать при отмене запроса.
    ///
    /// # Arguments
    ///
    /// * `require_file_match` - Требовать совпадение имени файла
    /// * `require_content_match` - Требовать совпадение в содержимом
    /// * `content` - Содержимое файла для чтения строк при отсутствии совпадений
    pub fn flush(&mut self, require_file_match: bool, require_content_match: bool, content: &[u8]) {
        // Исключаем файлы, помеченные как негативные на любом уровне
        if self.file_negated || self.content_negated {
            return;
        }

        // Проверяем требования к совпадению имени файла
        if require_file_match && !self.file_matched {
            return;
        }

        // Проверяем требования к совпадению содержимого
        if require_content_match && !self.content_matched {
            return;
        }

        // Определяем какие строки отправлять
        let lines = if self.content_matched {
            // Используем собранные строки с совпадениями
            std::mem::take(&mut self.lines)
        } else {
            // Читаем первые строки файла для контекста
            match read_lines(content) {
                Ok(lines) => lines,
                Err(e) => {
                    debug!("Failed to read file: {:?}", e);
                    vec![]
                }
            }
        };

        let file = GrepFile {
            path: self.path.clone(),
            lines,
        };

        // Отправляем результат, игнорируя ошибки отмены
        match self.tx.blocking_send(file) {
            Ok(_) => {}
            Err(_) => {
                // Запрос был отменен, можно безопасно игнорировать ошибку
            }
        }
    }
}

impl<'processor, 'pattern> Sink for GrepMatchSink<'processor, 'pattern> {
    type Error = std::io::Error;

    /// Обрабатывает найденное совпадение в файле
    ///
    /// Извлекает строку с совпадением, находит все вхождения паттерна
    /// и создает соответствующие GrepSubMatch объекты. Сохраняет
    /// обработанную строку в коллекцию результатов.
    ///
    /// # Arguments
    ///
    /// * `_searcher` - Searcher объект (не используется)
    /// * `mat` - Информация о найденном совпадении
    ///
    /// # Returns
    ///
    /// Ok(true) для продолжения поиска, Err для остановки
    fn matched<'matchspan>(
        &mut self,
        _searcher: &grep::searcher::Searcher,
        mat: &grep::searcher::SinkMatch<'matchspan>,
    ) -> Result<bool, Self::Error> {
        self.output.content_matched = true;

        // Поиск всегда выполняется в однострочном режиме
        let line = mat.lines().next().expect("Have at least one line");

        // Собираем все совпадения в строке для выделения
        let mut matches: Vec<GrepSubMatch> = vec![];
        self.matcher.find_iter(line, |m| {
            matches.push(GrepSubMatch {
                bytes_start: m.start(),
                bytes_end: m.end(),
            });
            true
        })?;

        // Кодируем строку в Base64 для безопасной передачи
        let line = GrepTextOrBase64::Base64(line.to_owned());

        // Создаем объект строки с метаданными и совпадениями
        let line = GrepLine {
            line,
            byte_offset: mat.absolute_byte_offset() as usize,
            line_number: mat.line_number().expect("Have line number") as usize,
            sub_matches: matches,
        };

        self.output.record(line);
        Ok(true)
    }

    /// Обрабатывает контекстную строку (без совпадений)
    ///
    /// Добавляет строки вокруг совпадений для предоставления контекста.
    /// Автоматически определяет кодировку и использует подходящий формат.
    ///
    /// # Arguments
    ///
    /// * `_searcher` - Searcher объект (не используется)
    /// * `context` - Информация о контекстной строке
    ///
    /// # Returns
    ///
    /// Ok(true) для продолжения поиска
    fn context<'vicinity>(
        &mut self,
        _searcher: &grep::searcher::Searcher,
        context: &grep::searcher::SinkContext<'vicinity>,
    ) -> Result<bool, Self::Error> {
        let line = context.bytes();

        // Пытаемся декодировать как UTF-8, иначе используем Base64
        let line = match std::str::from_utf8(line) {
            Ok(s) => GrepTextOrBase64::Text(s.to_owned()),
            Err(_) => GrepTextOrBase64::Base64(line.to_owned()),
        };

        self.output.record(GrepLine {
            line,
            byte_offset: context.absolute_byte_offset() as usize,
            line_number: context.line_number().expect("Have line number") as usize,
            sub_matches: vec![], // Контекстные строки не содержат совпадений
        });
        Ok(true)
    }
}

impl<'processor> Sink for GrepNegativeMatchSink<'processor> {
    type Error = std::io::Error;

    /// Обрабатывает совпадение в негативном поиске
    ///
    /// При первом совпадении помечает содержимое как исключенное
    /// и возвращает false для немедленной остановки поиска.
    /// Это оптимизация для быстрого исключения нежелательных файлов.
    ///
    /// # Arguments
    ///
    /// * `_searcher` - Searcher объект (не используется)
    /// * `_mat` - Информация о совпадении (не используется)
    ///
    /// # Returns
    ///
    /// Ok(false) для остановки поиска после первого совпадения
    fn matched<'matchspan>(
        &mut self,
        _searcher: &grep::searcher::Searcher,
        _mat: &grep::searcher::SinkMatch<'matchspan>,
    ) -> Result<bool, Self::Error> {
        self.output.content_negated = true;
        Ok(false) // Останавливаем поиск после первого совпадения
    }
}

// === FREE FUNCTIONS ===
/// Читает первые строки файла для предоставления контекста
///
/// Используется когда в файле не найдено совпадений, но нужно показать
/// его содержимое. Читает максимум 5 строк для предварительного просмотра
/// без перегрузки интерфейса большими объемами данных.
///
/// # Arguments
///
/// * `content` - Содержимое файла в виде байтов
///
/// # Returns
///
/// Вектор GrepLine объектов с первыми строками файла
///
/// # Errors
///
/// Возвращает ошибку если:
/// - Не удается прочитать содержимое как текст
/// - Ошибка при обработке строк
fn read_lines(content: &[u8]) -> anyhow::Result<Vec<GrepLine>> {
    let reader = std::io::BufReader::new(content);
    // Ограничиваем чтение первыми 5 строками для предварительного просмотра
    let line_reader = reader.lines().take(5);

    let mut lines = vec![];
    let mut line_number = 1;
    let mut byte_offset = 0;
    
    for line in line_reader {
        let line = line? + "\n"; // Восстанавливаем символ новой строки
        let bytes_length = line.len();
        
        lines.push(GrepLine {
            line: GrepTextOrBase64::Text(line),
            byte_offset,
            line_number,
            sub_matches: vec![], // Предварительный просмотр без выделения совпадений
        });

        byte_offset += bytes_length;
        line_number += 1;
    }

    Ok(lines)
}
