//! Build script для генерации метаданных сборки
//!
//! Использует vergen для генерации информации о сборке и git-репозитории.
//! Метаданные доступны в коде через environment variables во время компиляции.

// === IMPORTS ===
use std::error::Error;

use vergen::EmitBuilder;

// === FREE FUNCTIONS ===

/// Генерирует метаданные сборки и git-информацию
///
/// Использует vergen для создания переменных окружения времени компиляции,
/// содержащих информацию о сборке и состоянии git-репозитория.
/// 
/// # Returns
/// 
/// Возвращает `Ok(())` при успешной генерации метаданных.
/// 
/// # Errors
/// 
/// Возвращает ошибку если vergen не может получить информацию о git-репозитории
/// или записать переменные окружения.
fn main() -> Result<(), Box<dyn Error>> {
    EmitBuilder::builder()
        .all_build()
        .all_git()
        // TODO(kweizh): Временно отключаем match_pattern из-за проблемы на Windows
        // Вернём match_pattern когда проблема будет решена
        // ref: https://github.com/rustyhorde/vergen/issues/402
        .git_describe(false, true, None)
        .emit()?;
    
    Ok(())
}
