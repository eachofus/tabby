
//! Сервис автодополнения кода с поддержкой Retrieval Augmented Generation
//!
//! Предоставляет интеллектуальное автодополнение кода, используя модели генерации
//! и поиск релевантных сниппетов из кодовой базы для улучшения качества предложений.
//! Поддерживает стандартное автодополнение и режим предсказания следующих правок.

// === MODULES ===
mod completion_prompt;
mod next_edit_prompt;

// === IMPORTS ===
use std::sync::Arc;

use regex::Regex;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use utoipa::ToSchema;

use tabby_common::{
    api::{
        self,
        code::CodeSearch,
        event::{Event, EventLogger},
    },
    axum::AllowedCodeRepository,
    config::{CompletionConfig, ModelConfig},
    languages::get_language,
};
use tabby_inference::{
    ChatCompletionStream, CodeGeneration, CodeGenerationOptions, CodeGenerationOptionsBuilder,
    CompletionStream,
};

use super::model;

// === ENUMS ===
/// Ошибки сервиса автодополнения кода
///
/// Представляет все возможные ошибки, которые могут возникнуть
/// при обработке запросов на автодополнение кода.
#[derive(Error, Debug)]
pub enum CompletionError {
    /// Пустой промпт в запросе на автодополнение
    ///
    /// Возникает когда не предоставлены ни raw_prompt, ни segments
    #[error("empty prompt from completion request")]
    EmptyPrompt,
}

// === STRUCTS ===
/// Запрос на автодополнение кода
///
/// Содержит всю необходимую информацию для генерации автодополнения:
/// контекст кода, настройки модели, отладочные опции и режим работы.
/// Поддерживает как стандартное автодополнение, так и предсказание
/// следующих правок пользователя.
///
/// # Examples
///
/// ```rust
/// let request = CompletionRequest {
///     language: Some("rust".to_string()),
///     segments: Some(segments),
///     user: Some("user123".to_string()),
///     debug_options: None,
///     temperature: Some(0.7),
///     seed: None,
///     mode: "standard".to_string(),
/// };
/// ```
#[derive(Serialize, Deserialize, ToSchema, Clone, Debug)]
#[schema(example=json!({
    "language": "python",
    "segments": {
        "prefix": "def fib(n):\n    ",
        "suffix": "\n        return fib(n - 1) + fib(n - 2)"
    }
}))]
pub struct CompletionRequest {
    /// Идентификатор языка программирования
    ///
    /// Полный список поддерживаемых языков доступен по адресу:
    /// https://code.visualstudio.com/docs/languages/identifiers
    #[schema(example = "python")]
    language: Option<String>,

    /// Сегменты кода для автодополнения
    ///
    /// Когда указаны сегменты, поле `prompt` игнорируется при инференсе.
    /// Содержит префикс, суффикс и дополнительный контекст для генерации.
    segments: Option<Segments>,

    /// Уникальный идентификатор конечного пользователя
    ///
    /// Используется Tabby для мониторинга и генерации отчетов.
    /// Помогает отслеживать использование и качество автодополнений.
    pub(crate) user: Option<String>,

    /// Опции отладки для разработчиков
    ///
    /// Позволяют получить дополнительную информацию о процессе генерации,
    /// включая промпты, сниппеты и отключение RAG.
    debug_options: Option<DebugOptions>,

    /// Параметр температуры для модели
    ///
    /// Используется для настройки вариативности и "креативности" вывода модели.
    /// Значения от 0.0 (детерминированный) до 1.0 (максимально случайный).
    temperature: Option<f32>,

    /// Seed для случайного выбора токенов
    ///
    /// Обеспечивает воспроизводимость результатов при одинаковых входных данных.
    seed: Option<u64>,

    /// Режим автодополнения
    ///
    /// - 'standard' - обычное автодополнение кода
    /// - 'next_edit_suggestion' - предсказание следующей правки пользователя
    #[serde(default = "default_standard_mode")]
    mode: String,
}

/// Информация об истории правок для режима предсказания следующих правок
///
/// Содержит данные о предыдущих изменениях в файле для анализа
/// паттернов редактирования и предсказания следующих действий пользователя.
#[derive(Serialize, Deserialize, ToSchema, Clone, Debug)]
pub struct EditHistory {
    /// Оригинальный код до внесения изменений
    original_code: String,

    /// Унифицированный diff в стиле git всех внесенных правок
    ///
    /// Показывает все изменения, сделанные в файле с момента начала сессии.
    edits_diff: String,

    /// Текущая версия кода после всех правок
    ///
    /// Актуальное состояние файла на момент запроса предсказания.
    current_version: String,
}

/// Опции отладки для разработчиков и тестирования
///
/// Предоставляют дополнительную информацию о процессе генерации
/// автодополнений и позволяют настраивать поведение для тестирования.
#[derive(Serialize, Deserialize, ToSchema, Clone, Debug)]
pub struct DebugOptions {
    /// Прямой промпт для модели, минуя обработку сегментов
    ///
    /// Когда указан `raw_prompt`, он передается напрямую в движок инференса.
    /// Поле `segments` в `CompletionRequest` игнорируется.
    /// Полезно для тестирования качества модели end-to-end.
    raw_prompt: Option<String>,

    /// Возвращать сниппеты в отладочных данных
    ///
    /// Включает в ответ найденные релевантные сниппеты кода
    /// для анализа качества поиска и RAG.
    #[serde(default = "default_false")]
    return_snippets: bool,

    /// Возвращать промпт в отладочных данных
    ///
    /// Включает в ответ финальный промпт, отправленный в модель,
    /// для анализа и отладки процесса генерации.
    #[serde(default = "default_false")]
    return_prompt: bool,

    /// Отключить Retrieval Augmented Code Completion
    ///
    /// Когда true, автодополнение работает без поиска релевантных
    /// сниппетов кода, используя только контекст из сегментов.
    #[serde(default = "default_false")]
    disable_retrieval_augmented_code_completion: bool,
}

/// Сегменты кода для контекстного автодополнения
///
/// Содержит всю информацию о текущем состоянии редактора:
/// код до и после курсора, метаданные файла, релевантные сниппеты
/// и дополнительный контекст для улучшения качества автодополнения.
#[derive(Serialize, Deserialize, ToSchema, Clone, Debug)]
pub struct Segments {
    /// Содержимое, которое появляется перед курсором в окне редактора
    prefix: String,

    /// Содержимое, которое появляется после курсора в окне редактора
    suffix: Option<String>,

    /// Относительный путь редактируемого файла
    ///
    /// - Когда установлен [Segments::git_url], это путь файла в git репозитории
    /// - Когда [Segments::git_url] пуст, это путь файла в рабочем пространстве
    filepath: Option<String>,

    /// Удаленный URL текущего git репозитория
    ///
    /// Оставьте пустым, если файл не находится в git репозитории
    /// или репозиторий не имеет удаленного URL.
    git_url: Option<String>,

    /// Релевантные сниппеты объявлений, предоставленные LSP редактора
    ///
    /// Содержат объявления символов, извлеченных из [Segments::prefix].
    /// Помогают модели понять контекст используемых функций и типов.
    declarations: Option<Vec<Declaration>>,

    /// Релевантные сниппеты кода из недавно измененных файлов
    ///
    /// Сниппеты выбираются из кандидатов, найденных в чанках кода
    /// на основе места редактирования. Текущий редактируемый файл
    /// исключается из поиска кандидатов.
    ///
    /// При предоставлении вместе с [Segments::declarations] сниппеты
    /// уже дедуплицированы для исключения дублирования.
    ///
    /// Отсортированы в порядке убывания [Snippet::score].
    relevant_snippets_from_changed_files: Option<Vec<Snippet>>,

    /// Релевантные сниппеты кода из недавно открытых файлов
    ///
    /// Сниппеты выбираются из кандидатов, найденных в чанках кода
    /// на основе последнего посещенного места.
    ///
    /// Текущий активный файл исключается из поиска кандидатов.
    /// При предоставлении с [Segments::relevant_snippets_from_changed_files]
    /// сниппеты дедуплицированы.
    relevant_snippets_from_recently_opened_files: Option<Vec<Snippet>>,

    /// Содержимое буфера обмена при запросе автодополнения
    ///
    /// Может содержать релевантный код, скопированный пользователем,
    /// который следует учесть при генерации автодополнения.
    clipboard: Option<String>,

    /// Обязательно для режима 'next_edit_suggestion'
    ///
    /// Содержит информацию об истории правок для анализа паттернов
    /// и предсказания следующих действий пользователя.
    edit_history: Option<EditHistory>,
}

/// Сниппет объявления кода, релевантный для текущего запроса автодополнения
///
/// Представляет объявление функции, класса, переменной или другого символа,
/// который может быть полезен для понимания контекста автодополнения.
#[derive(Serialize, Deserialize, ToSchema, Clone, Debug)]
pub struct Declaration {
    /// Путь к файлу, где находится сниппет
    ///
    /// - Когда файл принадлежит тому же рабочему пространству, что и текущий файл,
    ///   это относительный путь, использующий то же правило, что и [Segments::filepath]
    /// - Когда файл находится вне рабочего пространства, например в пакете зависимостей,
    ///   это файловый URI с абсолютным путем
    pub filepath: String,

    /// Тело сниппета с кодом объявления
    ///
    /// Содержит полное объявление символа: сигнатуру функции,
    /// определение класса, объявление переменной и т.д.
    pub body: String,
}

/// Вариант автодополнения с индексом и текстом
///
/// Представляет один из возможных вариантов автодополнения,
/// сгенерированный моделью для данного контекста.
#[derive(Serialize, Deserialize, ToSchema, Clone, Debug)]
pub struct Choice {
    /// Индекс варианта в списке (обычно 0 для единственного варианта)
    index: u32,
    
    /// Текст автодополнения для вставки в редактор
    text: String,
}

/// Релевантный сниппет кода с оценкой релевантности
///
/// Представляет фрагмент кода из кодовой базы, который может быть
/// полезен для генерации автодополнения в текущем контексте.
#[derive(Serialize, Deserialize, ToSchema, Clone, Debug, PartialEq)]
pub struct Snippet {
    /// Путь к файлу, содержащему сниппет
    filepath: String,
    
    /// Тело сниппета с кодом
    body: String,
    
    /// Оценка релевантности сниппета (чем выше, тем релевантнее)
    score: f32,
}

/// Ответ сервиса автодополнения
///
/// Содержит сгенерированные варианты автодополнения, уникальный
/// идентификатор запроса и опциональные отладочные данные.
#[derive(Serialize, Deserialize, ToSchema, Clone, Debug)]
#[schema(example=json!({
    "id": "string",
    "choices": [ { "index": 0, "text": "string" } ]
}))]
pub struct CompletionResponse {
    /// Уникальный идентификатор запроса автодополнения
    id: String,
    
    /// Список вариантов автодополнения
    choices: Vec<Choice>,

    /// Отладочные данные (включаются только при соответствующих опциях)
    #[serde(skip_serializing_if = "Option::is_none")]
    debug_data: Option<DebugData>,

    /// Режим автодополнения, использованный для генерации
    #[serde(default = "default_standard_mode")]
    mode: String,
}

/// Отладочные данные для анализа процесса автодополнения
///
/// Содержит дополнительную информацию о том, как было сгенерировано
/// автодополнение: использованные сниппеты и финальный промпт.
#[derive(Serialize, Deserialize, ToSchema, Clone, Debug)]
pub struct DebugData {
    /// Сниппеты кода, использованные для RAG (если запрошены)
    #[serde(skip_serializing_if = "Option::is_none")]
    snippets: Option<Vec<Snippet>>,

    /// Финальный промпт, отправленный в модель (если запрошен)
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt: Option<String>,
}

/// Сервис автодополнения с поддержкой Retrieval Augmented Generation
///
/// Расширяет возможности генерации кода за счет поиска релевантных
/// сниппетов из кодовой базы, которые затем используются как контекст
/// для модели генерации кода. Поддерживает различные режимы работы
/// и гибкую настройку параметров генерации.
///
/// # Architecture
///
/// Сервис состоит из нескольких компонентов:
/// - Движок генерации кода (CodeGeneration)
/// - Поисковый движок (CodeSearch) для RAG
/// - Построители промптов для разных режимов
/// - Система логирования событий
///
/// # Examples
///
/// ```rust
/// let service = CompletionService::new(
///     config,
///     engine,
///     code_search,
///     logger,
///     Some(prompt_template),
/// );
///
/// let response = service.generate(
///     &request,
///     &allowed_repos,
///     Some("VSCode/1.0"),
/// ).await?;
/// ```
pub struct CompletionService {
    /// Конфигурация сервиса автодополнения
    config: CompletionConfig,
    
    /// Движок генерации кода
    engine: Arc<CodeGeneration>,
    
    /// Система логирования событий для аналитики
    logger: Arc<dyn EventLogger>,
    
    /// Построитель промптов для стандартного режима
    prompt_builder: completion_prompt::PromptBuilder,
    
    /// Построитель промптов для режима предсказания правок
    next_edit_prompt_builder: next_edit_prompt::NextEditPromptBuilder,
}

// === IMPLEMENTATIONS ===
impl CompletionRequest {
    /// Возвращает язык программирования или "unknown" если не указан
    ///
    /// Используется для настройки специфичных для языка параметров
    /// генерации и поиска релевантных сниппетов.
    fn language_or_unknown(&self) -> String {
        self.language.clone().unwrap_or("unknown".to_string())
    }

    /// Возвращает сырой промпт если указан в отладочных опциях
    ///
    /// Когда установлен, обходит всю обработку сегментов и отправляет
    /// промпт напрямую в модель для тестирования и отладки.
    fn raw_prompt(&self) -> Option<String> {
        self.debug_options
            .as_ref()
            .and_then(|x| x.raw_prompt.clone())
    }

    /// Проверяет, отключен ли Retrieval Augmented Code Completion
    ///
    /// Когда true, автодополнение работает без поиска релевантных
    /// сниппетов, используя только предоставленный контекст.
    fn disable_retrieval_augmented_code_completion(&self) -> bool {
        self.debug_options
            .as_ref()
            .is_some_and(|x| x.disable_retrieval_augmented_code_completion)
    }

    /// Проверяет, является ли запрос режимом предсказания следующих правок
    ///
    /// В этом режиме система анализирует историю правок пользователя
    /// и предсказывает наиболее вероятные следующие изменения.
    fn is_next_edit_suggestion_mode(&self) -> bool {
        self.mode == "next_edit_suggestion"
    }
}

impl From<Segments> for api::event::Segments {
    /// Преобразует сегменты запроса в формат для логирования событий
    ///
    /// Сохраняет основные поля и преобразует вложенные структуры
    /// для единообразного логирования в системе аналитики.
    fn from(val: Segments) -> Self {
        Self {
            prefix: val.prefix,
            suffix: val.suffix,
            clipboard: val.clipboard,
            git_url: val.git_url,
            declarations: val
                .declarations
                .map(|x| x.into_iter().map(Into::into).collect()),
            filepath: val.filepath,
        }
    }
}

impl From<Declaration> for api::event::Declaration {
    /// Преобразует объявление в формат для логирования событий
    fn from(val: Declaration) -> Self {
        Self {
            filepath: val.filepath,
            body: val.body,
        }
    }
}

impl Choice {
    /// Создает новый вариант автодополнения с индексом 0
    ///
    /// # Arguments
    ///
    /// * `text` - Текст автодополнения для вставки
    pub fn new(text: String) -> Self {
        Self { index: 0, text }
    }
}

impl CompletionResponse {
    /// Создает новый ответ автодополнения
    ///
    /// # Arguments
    ///
    /// * `id` - Уникальный идентификатор запроса
    /// * `choices` - Варианты автодополнения
    /// * `debug_data` - Отладочные данные (опционально)
    /// * `mode` - Режим автодополнения
    pub fn new(
        id: String,
        choices: Vec<Choice>,
        debug_data: Option<DebugData>,
        mode: String,
    ) -> Self {
        Self {
            id,
            choices,
            debug_data,
            mode,
        }
    }
}

impl CompletionService {
    /// Создает новый сервис автодополнения
    ///
    /// Инициализирует все компоненты сервиса: движок генерации,
    /// построители промптов, систему логирования и поисковый движок.
    ///
    /// # Arguments
    ///
    /// * `config` - Конфигурация сервиса
    /// * `engine` - Движок генерации кода
    /// * `code` - Поисковый движок для RAG
    /// * `logger` - Система логирования событий
    /// * `prompt_template` - Шаблон промпта для стандартного режима
    fn new(
        config: CompletionConfig,
        engine: Arc<CodeGeneration>,
        code: Arc<dyn CodeSearch>,
        logger: Arc<dyn EventLogger>,
        prompt_template: Option<String>,
    ) -> Self {
        Self {
            engine,
            prompt_builder: completion_prompt::PromptBuilder::new(
                &config.code_search_params,
                prompt_template,
                Some(code),
            ),
            next_edit_prompt_builder: next_edit_prompt::NextEditPromptBuilder::new(),
            config,
            logger,
        }
    }

    /// Собирает релевантные сниппеты кода для RAG
    ///
    /// Выполняет поиск по кодовой базе для нахождения фрагментов,
    /// релевантных текущему контексту автодополнения. Учитывает
    /// язык программирования и разрешенные репозитории.
    ///
    /// # Arguments
    ///
    /// * `language` - Язык программирования
    /// * `segments` - Сегменты кода для анализа контекста
    /// * `allowed_code_repository` - Разрешенные репозитории для поиска
    /// * `disable_retrieval_augmented_code_completion` - Флаг отключения RAG
    ///
    /// # Returns
    ///
    /// Вектор релевантных сниппетов, отсортированных по релевантности
    async fn build_snippets(
        &self,
        language: &str,
        segments: &Segments,
        allowed_code_repository: &AllowedCodeRepository,
        disable_retrieval_augmented_code_completion: bool,
    ) -> Vec<Snippet> {
        if disable_retrieval_augmented_code_completion {
            return vec![];
        }

        self.prompt_builder
            .collect(language, segments, allowed_code_repository)
            .await
    }

    /// Создает опции для генерации текста
    ///
    /// Настраивает параметры модели генерации на основе запроса:
    /// температуру, seed, ограничения по длине и специфичные
    /// для языка настройки.
    ///
    /// # Arguments
    ///
    /// * `language` - Язык программирования
    /// * `temperature` - Параметр температуры модели
    /// * `seed` - Seed для воспроизводимости
    /// * `max_input_length` - Максимальная длина входа
    /// * `max_output_tokens` - Максимальное количество выходных токенов
    /// * `mode` - Режим генерации
    ///
    /// # Returns
    ///
    /// Настроенные опции генерации кода
    fn text_generation_options(
        language: &str,
        temperature: Option<f32>,
        seed: Option<u64>,
        max_input_length: usize,
        max_output_tokens: usize,
        mode: String,
    ) -> CodeGenerationOptions {
        let mut builder = CodeGenerationOptionsBuilder::default();
        builder
            .max_input_length(max_input_length)
            .max_decoding_tokens(max_output_tokens as i32)
            .language(Some(get_language(language)));
        
        // Настраиваем температуру если указана
        temperature.inspect(|x| {
            builder.sampling_temperature(*x);
        });
        
        // Настраиваем seed если указан
        seed.inspect(|x| {
            builder.seed(*x);
        });

        builder.mode(mode);

        builder
            .build()
            .expect("Failed to create text generation options")
    }

    /// Генерирует автодополнение кода
    ///
    /// Основной метод сервиса, который обрабатывает запрос на автодополнение:
    /// анализирует контекст, собирает релевантные сниппеты, строит промпт,
    /// генерирует код и логирует результат.
    ///
    /// # Arguments
    ///
    /// * `request` - Запрос на автодополнение
    /// * `allowed_code_repository` - Разрешенные репозитории
    /// * `user_agent` - User agent клиента для аналитики
    ///
    /// # Returns
    ///
    /// Ответ с вариантами автодополнения и отладочными данными
    ///
    /// # Errors
    ///
    /// Возвращает `CompletionError::EmptyPrompt` если не предоставлен
    /// ни raw_prompt, ни segments
    pub async fn generate(
        &self,
        request: &CompletionRequest,
        allowed_code_repository: &AllowedCodeRepository,
        user_agent: Option<&str>,
    ) -> Result<CompletionResponse, CompletionError> {
        let completion_id = format!("cmpl-{}", uuid::Uuid::new_v4());
        let language = request.language_or_unknown();

        // Обрабатываем режим предсказания следующих правок отдельно
        if request.is_next_edit_suggestion_mode() {
            return self
                .generate_next_edit_suggestion(request, completion_id, language, user_agent)
                .await;
        }

        let options = Self::text_generation_options(
            language.as_str(),
            request.temperature,
            request.seed,
            self.config.max_input_length,
            self.config.max_decoding_tokens,
            request.mode.clone(),
        );

        let mut use_crlf = false;
        let (prompt, segments, snippets) = if let Some(prompt) = request.raw_prompt() {
            // Используем сырой промпт без обработки
            (prompt, None, vec![])
        } else if let Some(segments) = request.segments.as_ref() {
            // Проверяем использование CRLF для корректной обработки переносов строк
            if contains_crlf(segments) {
                use_crlf = true;
            }

            // Собираем релевантные сниппеты для RAG
            let snippets = self
                .build_snippets(
                    &language,
                    segments,
                    allowed_code_repository,
                    request.disable_retrieval_augmented_code_completion(),
                )
                .await;
            
            // Строим финальный промпт с учетом сниппетов
            let prompt = self
                .prompt_builder
                .build(&language, segments.clone(), &snippets);

            (override_prompt(prompt, use_crlf), Some(segments), snippets)
        } else {
            return Err(CompletionError::EmptyPrompt);
        };

        // Генерируем код с учетом настроек переносов строк
        let generated_text =
            override_generated_text(self.engine.generate(&prompt, options).await, use_crlf);
            
        // Логируем событие для аналитики
        self.logger.log(
            request.user.clone(),
            Event::Completion {
                completion_id: completion_id.clone(),
                language,
                prompt: prompt.clone(),
                segments: segments.cloned().map(|x| x.into()),
                choices: vec![api::event::Choice {
                    index: 0,
                    text: generated_text.clone(),
                }],
                user_agent: user_agent.map(|x| x.to_owned()),
            },
        );

        // Подготавливаем отладочные данные если запрошены
        let debug_data = request
            .debug_options
            .as_ref()
            .map(|debug_options| DebugData {
                snippets: debug_options.return_snippets.then_some(snippets),
                prompt: debug_options.return_prompt.then_some(prompt),
            });

        Ok(CompletionResponse::new(
            completion_id,
            vec![Choice::new(generated_text)],
            debug_data,
            "standard".to_string(),
        ))
    }

    /// Генерирует предсказание следующей правки пользователя
    ///
    /// Специальный режим, который анализирует историю правок пользователя
    /// и предсказывает наиболее вероятные следующие изменения в коде.
    /// Использует увеличенный лимит токенов для более детального анализа.
    ///
    /// # Arguments
    ///
    /// * `request` - Запрос на автодополнение
    /// * `completion_id` - Уникальный идентификатор запроса
    /// * `language` - Язык программирования
    /// * `user_agent` - User agent клиента
    ///
    /// # Returns
    ///
    /// Ответ с предсказанием следующей правки
    ///
    /// # Errors
    ///
    /// Возвращает `CompletionError::EmptyPrompt` если отсутствуют
    /// segments или edit_history
    async fn generate_next_edit_suggestion(
        &self,
        request: &CompletionRequest,
        completion_id: String,
        language: String,
        user_agent: Option<&str>,
    ) -> Result<CompletionResponse, CompletionError> {
        let segments = request
            .segments
            .as_ref()
            .ok_or(CompletionError::EmptyPrompt)?;

        let edit_history = segments
            .edit_history
            .as_ref()
            .ok_or(CompletionError::EmptyPrompt)?;

        // Строим специализированный промпт для анализа истории правок
        let prompt = self.next_edit_prompt_builder.build_prompt(edit_history);

        // Используем увеличенный лимит токенов для более детального анализа
        let options = Self::text_generation_options(
            language.as_str(),
            request.temperature,
            request.seed,
            self.config.max_input_length,
            self.config.max_decoding_tokens * 2, // Удваиваем лимит для анализа правок
            request.mode.clone(),
        );

        let generated_text = self.engine.generate(&prompt, options).await;

        // Логируем событие предсказания правки
        self.logger.log(
            request.user.clone(),
            Event::Completion {
                completion_id: completion_id.clone(),
                language,
                prompt: prompt.clone(),
                segments: None, // Не логируем segments для режима правок
                choices: vec![api::event::Choice {
                    index: 0,
                    text: generated_text.clone(),
                }],
                user_agent: user_agent.map(|x| x.to_owned()),
            },
        );

        let debug_data = request
            .debug_options
            .as_ref()
            .map(|debug_options| DebugData {
                snippets: None, // Сниппеты не используются в режиме правок
                prompt: debug_options.return_prompt.then_some(prompt),
            });

        Ok(CompletionResponse::new(
            completion_id,
            vec![Choice::new(generated_text)],
            debug_data,
            "next_edit_suggestion".to_string(),
        ))
    }
}

// === FREE FUNCTIONS ===
/// Возвращает стандартный режим автодополнения по умолчанию
///
/// Используется как значение по умолчанию для поля mode в serde.
pub fn default_standard_mode() -> String {
    "standard".to_string()
}

/// Возвращает false как значение по умолчанию
///
/// Используется для булевых полей в отладочных опциях.
fn default_false() -> bool {
    false
}

/// Проверяет наличие CRLF переносов строк в сегментах
///
/// Анализирует prefix и suffix на предмет использования Windows-стиля
/// переносов строк (\r\n) для корректной обработки генерируемого текста.
///
/// # Arguments
///
/// * `segments` - Сегменты кода для проверки
///
/// # Returns
///
/// true если найдены CRLF переносы, false иначе
fn contains_crlf(segments: &Segments) -> bool {
    if segments.prefix.contains("\r\n") {
        return true;
    }
    if let Some(suffix) = &segments.suffix {
        if suffix.contains("\r\n") {
            return true;
        }
    }

    false
}

/// Преобразует переносы строк в промпте при необходимости
///
/// Нормализует CRLF переносы в LF для единообразной обработки моделью.
/// Это необходимо, поскольку модели обычно обучены на LF переносах.
///
/// # Arguments
///
/// * `prompt` - Исходный промпт
/// * `use_crlf` - Флаг использования CRLF в исходном коде
///
/// # Returns
///
/// Промпт с нормализованными переносами строк
fn override_prompt(prompt: String, use_crlf: bool) -> String {
    if use_crlf {
        prompt.replace("\r\n", "\n")
    } else {
        prompt
    }
}

/// Преобразует переносы строк в сгенерированном тексте
///
/// Заменяет \n на \r\n в сгенерированном тексте если use_crlf равно true.
/// Это обеспечивает соответствие стиля переносов строк исходному коду.
///
/// Поскольку в тексте могут уже присутствовать \r\n последовательности,
/// которые также содержат `\n` и не должны заменяться, мы не можем
/// просто заменить \n на \r\n. Используется regex для точного поиска.
///
/// # Arguments
///
/// * `generated` - Сгенерированный текст
/// * `use_crlf` - Флаг использования CRLF стиля
///
/// # Returns
///
/// Текст с корректными переносами строк
fn override_generated_text(generated: String, use_crlf: bool) -> String {
    if use_crlf {
        // Находим \n, которым не предшествует \r
        let re = Regex::new(r"([^\r])\n").unwrap();
        re.replace_all(&generated, "$1\r\n").to_string()
    } else {
        generated
    }
}

/// Создает сервис автодополнения и чат-сервисы
///
/// Фабричная функция для инициализации всех компонентов системы
/// автодополнения: основного сервиса, потокового автодополнения
/// и чат-сервиса. Загружает модели и настраивает все зависимости.
///
/// # Arguments
///
/// * `config` - Конфигурация автодополнения
/// * `code` - Поисковый движок для RAG
/// * `logger` - Система логирования событий
/// * `completion` - Конфигурация модели автодополнения
/// * `chat` - Конфигурация чат-модели
///
/// # Returns
///
/// Кортеж из опциональных сервисов:
/// - CompletionService для основного автодополнения
/// - CompletionStream для потокового автодополнения
/// - ChatCompletionStream для чат-функциональности
pub async fn create_completion_service_and_chat(
    config: &CompletionConfig,
    code: Arc<dyn CodeSearch>,
    logger: Arc<dyn EventLogger>,
    completion: Option<ModelConfig>,
    chat: Option<ModelConfig>,
) -> (
    Option<CompletionService>,
    Option<Arc<dyn CompletionStream>>,
    Option<Arc<dyn ChatCompletionStream>>,
) {
    let (code_generation, completion_stream, chat, prompt) =
        model::load_code_generation_and_chat(completion, chat).await;

    let completion = code_generation.clone().map(|code_generation| {
        CompletionService::new(
            config.to_owned(),
            code_generation.clone(),
            code,
            logger,
            prompt
                .unwrap_or_else(|| panic!("Prompt template is required for code completion"))
                .prompt_template,
        )
    });

    (completion, completion_stream, chat)
}

// === TESTS ===
#[cfg(test)]
mod tests {
    use async_stream::stream;
    use async_trait::async_trait;
    use futures::stream::BoxStream;

    use tabby_common::{
        api::{
            code::{CodeSearchError, CodeSearchParams, CodeSearchQuery, CodeSearchResponse},
            event,
        },
        axum::AllowedCodeRepository,
    };
    use tabby_inference::{CompletionOptions, CompletionStream};

    use super::*;

    /// Mock реализация системы логирования для тестов
    struct MockEventLogger;

    impl EventLogger for MockEventLogger {
        fn write(&self, _x: event::LogEntry) {}
    }

    /// Mock реализация потокового автодополнения для тестов
    struct MockCompletionStream;

    #[async_trait]
    impl CompletionStream for MockCompletionStream {
        #[allow(mismatched_lifetime_syntaxes)]
        async fn generate(&self, _prompt: &'_ str, _options: CompletionOptions) -> BoxStream<String> {
            let s = stream! {
                yield r#""Hello, world!""#.into();
            };

            Box::pin(s)
        }
    }

    /// Mock реализация поиска кода для тестов
    struct MockCodeSearch;

    #[async_trait]
    impl CodeSearch for MockCodeSearch {
        async fn search_in_language(
            &self,
            _query: CodeSearchQuery,
            _params: CodeSearchParams,
        ) -> Result<CodeSearchResponse, CodeSearchError> {
            Ok(CodeSearchResponse { hits: vec![] })
        }
    }

    /// Создает mock сервис автодополнения для тестирования
    fn mock_completion_service() -> CompletionService {
        let generation = CodeGeneration::new(Arc::new(MockCompletionStream), None);
        CompletionService::new(
            CompletionConfig::default(),
            Arc::new(generation),
            Arc::new(MockCodeSearch),
            Arc::new(MockEventLogger),
            Some("<pre>{prefix}<mid>{suffix}<end>".into()),
        )
    }

    #[tokio::test]
    async fn test_completion_service() {
        let completion_service = mock_completion_service();
        let segment = Segments {
            prefix: "fn hello_world() -> &'static str {".into(),
            suffix: Some("}".into()),
            filepath: None,
            git_url: None,
            declarations: None,
            relevant_snippets_from_changed_files: None,
            relevant_snippets_from_recently_opened_files: None,
            clipboard: None,
            edit_history: None,
        };
        let request = CompletionRequest {
            language: Some("rust".into()),
            segments: Some(segment.clone()),
            user: None,
            debug_options: None,
            temperature: None,
            seed: None,
            mode: "standard".into(),
        };

        let allowed_code_repository = AllowedCodeRepository::default();
        let response = completion_service
            .generate(&request, &allowed_code_repository, Some("test user agent"))
            .await
            .unwrap();
        assert_eq!(response.choices[0].text, r#""Hello, world!""#);

        let prompt = completion_service
            .prompt_builder
            .build("rust", segment.clone(), &[]);
        assert_eq!(prompt, "<pre>fn hello_world() -> &'static str {<mid>}<end>");
    }

    #[test]
    fn test_contains_crlf() {
        let contained_crlf = vec![
            Segments {
                prefix: "fn hello_world() -> &'static str {\r\n".into(),
                suffix: Some("}".into()),
                filepath: None,
                git_url: None,
                declarations: None,
                relevant_snippets_from_changed_files: None,
                relevant_snippets_from_recently_opened_files: None,
                clipboard: None,
                edit_history: None,
            },
            Segments {
                prefix: "fn hello_world() -> &'static str {".into(),
                suffix: Some("}\r\n".into()),
                filepath: None,
                git_url: None,
                declarations: None,
                relevant_snippets_from_changed_files: None,
                relevant_snippets_from_recently_opened_files: None,
                clipboard: None,
                edit_history: None,
            },
            Segments {
                prefix: "fn hello_world() -> &'static str {\r\n".into(),
                suffix: Some("}\r\n".into()),
                filepath: None,
                git_url: None,
                declarations: None,
                relevant_snippets_from_changed_files: None,
                relevant_snippets_from_recently_opened_files: None,
                clipboard: None,
                edit_history: None,
            },
        ];
        for segments in contained_crlf {
            assert!(contains_crlf(&segments));
        }

        let not_contained_crlf = vec![Segments {
            prefix: "fn hello_world() -> &'static str {\r".into(),
            suffix: Some("}\n".into()),
            filepath: None,
            git_url: None,
            declarations: None,
            relevant_snippets_from_changed_files: None,
            relevant_snippets_from_recently_opened_files: None,
            clipboard: None,
            edit_history: None,
        }];
        for segments in not_contained_crlf {
            assert!(!contains_crlf(&segments));
        }
    }

    #[test]
    fn test_override_prompt() {
        let prompt = "fn hello_world() -> &'static str {\r\n".to_string();
        let use_crlf = true;
        assert_eq!(
            override_prompt(prompt.clone(), use_crlf),
            "fn hello_world() -> &'static str {\n"
        );

        let use_crlf = false;
        assert_eq!(override_prompt(prompt.clone(), use_crlf), prompt);
    }

    #[test]
    fn test_override_generated() {
        let cases = vec![
            (
                "fn hello_world() -> &'static str {\r\n".to_string(),
                "fn hello_world() -> &'static str {\r\n".to_string(),
            ),
            (
                "fn hello_world() -> &'static str {\n".to_string(),
                "fn hello_world() -> &'static str {\r\n".to_string(),
            ),
            (
                "fn hello_world() -> &'static str {\r".to_string(),
                "fn hello_world() -> &'static str {\r".to_string(),
            ),
            (
                "fn hello_world() -> &'static str {".to_string(),
                "fn hello_world() -> &'static str {".to_string(),
            ),
        ];

        for (generated, expected) in cases {
            assert_eq!(override_generated_text(generated, true), expected);
        }
    }
}