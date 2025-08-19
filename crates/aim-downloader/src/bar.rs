// === IMPORTS ===
use std::{collections::HashMap, env};

use dotenvy::dotenv;
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use strfmt::strfmt;

// === CONSTANTS ===
const DEFAULT_AIM_PROGRESSBAR_DOWNLOADED_MESSAGE: &str = "🎯 Downloaded {input} to {output}";
const DEFAULT_AIM_PROGRESSBAR_MESSAGE_FORMAT: &str = "🎯 Transferring {url}";
const DEFAULT_AIM_PROGRESSBAR_PROGRESS_CHARS: &str = "█▉▊▋▌▍▎▏  ";
const DEFAULT_AIM_PROGRESSBAR_TEMPLATE: &str = "{msg}\n{spinner:.cyan}  {elapsed_precise} ▕{bar:.white}▏ {bytes}/{total_bytes}  {bytes_per_sec}  ETA {eta}.";
const DEFAULT_AIM_PROGRESSBAR_UPLOADED_MESSAGE: &str = "🎯 Uploaded {input} to {output}";
const THRESHOLD_IF_TOTALBYTES_BELOW_THEN_AUTO_SILENT_MODE: u64 = 1024 * 1024;

// === STRUCTS ===
/// Wrapper around indicatif::ProgressBar with enhanced functionality
/// 
/// Provides a configurable progress bar that can be silenced based on file size
/// and supports custom messages for download/upload completion.
pub struct WrappedBar {
    pub silent: bool,
    pub output: Option<indicatif::ProgressBar>,
    downloaded_message: String,
    uploaded_message: String,
}

// === IMPLEMENTATIONS ===
impl WrappedBar {
    /// Creates a new empty WrappedBar in silent mode
    pub fn new_empty() -> Self {
        WrappedBar {
            silent: true,
            output: None,
            downloaded_message: String::new(),
            uploaded_message: String::new(),
        }
    }

    /// Creates a new empty WrappedBar in verbose mode
    pub fn new_empty_verbose() -> Self {
        WrappedBar {
            silent: false,
            output: None,
            downloaded_message: String::new(),
            uploaded_message: String::new(),
        }
    }

    /// Creates a new WrappedBar with specified parameters
    /// 
    /// # Arguments
    /// * `total_size` - Total size of the operation in bytes
    /// * `url` - URL being processed (used in progress messages)
    /// * `silent` - Whether to suppress progress bar output
    /// 
    /// # Returns
    /// A new WrappedBar instance configured with environment variables or defaults
    pub fn new(total_size: u64, url: &str, silent: bool) -> Self {
        dotenv().ok();
        let message_format = &env::var("AIM_PROGRESSBAR_MESSAGE_FORMAT")
            .unwrap_or_else(|_| DEFAULT_AIM_PROGRESSBAR_MESSAGE_FORMAT.to_string());
        let progress_chars = &env::var("AIM_PROGRESSBAR_PROGRESS_CHARS")
            .unwrap_or_else(|_| DEFAULT_AIM_PROGRESSBAR_PROGRESS_CHARS.to_string());
        let template = &env::var("AIM_PROGRESSBAR_TEMPLATE")
            .unwrap_or_else(|_| DEFAULT_AIM_PROGRESSBAR_TEMPLATE.to_string());
        let downloaded_message = &env::var("AIM_PROGRESSBAR_DOWNLOADED_MESSAGE")
            .unwrap_or_else(|_| DEFAULT_AIM_PROGRESSBAR_DOWNLOADED_MESSAGE.to_string());
        let uploaded_message = &env::var("AIM_PROGRESSBAR_UPLOADED_MESSAGE")
            .unwrap_or_else(|_| DEFAULT_AIM_PROGRESSBAR_UPLOADED_MESSAGE.to_string());
        
        let output = match silent {
            false => Some(construct_progress_bar(
                total_size,
                url,
                message_format,
                progress_chars,
                template,
            )),
            true => None,
        };
        
        WrappedBar {
            silent,
            output,
            downloaded_message: downloaded_message.to_string(),
            uploaded_message: uploaded_message.to_string(),
        }
    }

    /// Sets the total length of the progress bar
    /// 
    /// Automatically enables silent mode if the length is below the threshold.
    /// 
    /// # Arguments
    /// * `len` - Total length in bytes
    pub fn set_length(&mut self, len: u64) {
        if len < THRESHOLD_IF_TOTALBYTES_BELOW_THEN_AUTO_SILENT_MODE {
            self.silent = true;
        }
        if !self.silent {
            self.output
                .as_ref()
                .unwrap()
                .set_draw_target(ProgressDrawTarget::stderr());
            self.output.as_ref().unwrap().set_length(len);
        }
    }

    /// Sets the current position of the progress bar
    /// 
    /// # Arguments
    /// * `pos` - Current position in bytes
    pub fn set_position(&self, pos: u64) {
        if !self.silent {
            self.output.as_ref().unwrap().set_position(pos);
        }
    }

    /// Finishes the progress bar with a download completion message
    /// 
    /// # Arguments
    /// * `input` - Input source path/URL
    /// * `output` - Output destination path
    pub fn finish_download(&self, input: &str, output: &str) {
        if !self.silent {
            let mut vars: HashMap<String, String> = HashMap::new();

            if self.downloaded_message.contains("{input}") {
                vars.insert("input".to_string(), input.to_string());
            }

            if self.downloaded_message.contains("{output}") {
                vars.insert("output".to_string(), output.to_string());
            }
            
            self.output
                .as_ref()
                .unwrap()
                .finish_with_message(strfmt(&self.downloaded_message, &vars).unwrap());
        }
    }

    /// Finishes the progress bar with an upload completion message
    /// 
    /// # Arguments
    /// * `input` - Input source path
    /// * `output` - Output destination URL/path
    pub fn finish_upload(&self, input: &str, output: &str) {
        if !self.silent {
            let mut vars: HashMap<String, String> = HashMap::new();

            if self.uploaded_message.contains("{input}") {
                vars.insert("input".to_string(), input.to_string());
            }

            if self.uploaded_message.contains("{output}") {
                vars.insert("output".to_string(), output.to_string());
            }
            
            self.output
                .as_ref()
                .unwrap()
                .finish_with_message(strfmt(&self.uploaded_message, &vars).unwrap());
        }
    }
}

// === FREE FUNCTIONS ===
/// Constructs a configured progress bar with specified parameters
/// 
/// # Arguments
/// * `total_size` - Total size in bytes
/// * `url` - URL for message formatting
/// * `message_format` - Format string for progress messages
/// * `progress_chars` - Characters used for progress visualization
/// * `template` - Template for progress bar layout
/// 
/// # Returns
/// A configured indicatif::ProgressBar instance
fn construct_progress_bar(
    total_size: u64,
    url: &str,
    message_format: &str,
    progress_chars: &str,
    template: &str,
) -> indicatif::ProgressBar {
    let pb = ProgressBar::new(total_size);
    pb.set_draw_target(ProgressDrawTarget::hidden());
    let mut vars: HashMap<String, String> = HashMap::new();

    if message_format.contains("{url}") {
        vars.insert("url".to_string(), url.to_string());
    }

    pb.set_message(strfmt(message_format, &vars).unwrap());
    pb.set_style(
        ProgressStyle::default_bar()
            .template(template)
            .unwrap()
            .progress_chars(progress_chars),
    );
    pb
}

// === TESTS ===
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bar_finish_download_works_when_typical() {
        let bar = WrappedBar::new(42, "url", false);
        bar.finish_download("", "");
    }

    #[test]
    fn test_bar_finish_upload_works_when_typical() {
        let bar = WrappedBar::new(42, "url", false);
        bar.finish_upload("", "");
    }

    #[test]
    fn test_bar_set_length_works_when_typical() {
        let mut bar = WrappedBar::new(42, "url", false);
        bar.set_length(42);
    }
}
