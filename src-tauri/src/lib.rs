use anyhow::Result;
use pdf_extract::extract_text;
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;
use rayon::prelude::*;
use std::sync::{Arc, Mutex};
use std::path::Path;
use tauri::Emitter;
use std::collections::HashMap;
use std::sync::RwLock;

#[derive(Debug, Serialize, Clone)]
struct SearchResult {
    file_path: String,
    matched_text: String,
    file_size: u64,
}

#[derive(Debug, Deserialize)]
struct SearchConfig {
    folder_path: String,
    keyword: String,
}

#[derive(Debug, Serialize)]
struct SearchProgress {
    current: usize,
    total: usize,
    current_file: String,
}

// 添加原来的 search_in_pdf 函数以支持向后兼容
fn extract_pdf_text(file_path: &str) -> Result<String> {
    let text = extract_text(file_path)?;
    Ok(text)
}

fn search_in_pdf(file_path: &str, keyword: &str) -> Result<Option<SearchResult>> {
    let path = Path::new(file_path);
    let file_size = path.metadata()?.len();
    
    let text = extract_pdf_text(file_path)?;
    
    // 优化搜索：使用更高效的字符串搜索
    let keyword_lower = keyword.to_lowercase();
    if text.to_lowercase().contains(&keyword_lower) {
        // 找到包含关键词的上下文，提供更多上下文
        let lines: Vec<&str> = text.lines().collect();
        let mut context_lines = Vec::new();
        
        for (i, line) in lines.iter().enumerate() {
            if line.to_lowercase().contains(&keyword_lower) {
                // 包含前后各1行作为上下文
                let start = if i > 0 { i - 1 } else { 0 };
                let end = if i + 1 < lines.len() { i + 2 } else { lines.len() };
                
                for j in start..end {
                    if !context_lines.contains(&lines[j]) {
                        context_lines.push(lines[j]);
                    }
                }
                break; // 只获取第一个匹配的上下文
            }
        }
        
        let context = context_lines.join("\n").trim().to_string();
        let context = if context.len() > 200 {
            format!("{}...", context.chars().take(100).collect::<String>())
        } else {
            context
        };

        Ok(Some(SearchResult {
            file_path: file_path.to_string(),
            matched_text: context,
            file_size,
        }))
    } else {
        Ok(None)
    }
}

// 简单的内存缓存
lazy_static::lazy_static! {
    static ref TEXT_CACHE: RwLock<HashMap<String, String>> = RwLock::new(HashMap::new());
}

// 优化的文本提取函数，带缓存
fn extract_pdf_text_cached(file_path: &str) -> Result<String> {
    // 检查缓存
    {
        let cache = TEXT_CACHE.read().unwrap();
        if let Some(cached_text) = cache.get(file_path) {
            return Ok(cached_text.clone());
        }
    }
    
    // 如果没有缓存，提取文本
    let text = extract_text(file_path)?;
    
    // 存入缓存（限制缓存大小）
    {
        let mut cache = TEXT_CACHE.write().unwrap();
        if cache.len() < 100 { // 限制缓存条目数
            cache.insert(file_path.to_string(), text.clone());
        }
    }
    
    Ok(text)
}

// 快速搜索函数，支持多个关键词
fn quick_search_in_pdf(file_path: &str, keywords: &[String]) -> Result<Option<SearchResult>> {
    let path = Path::new(file_path);
    let file_size = path.metadata()?.len();
    
    // 使用缓存的文本提取
    let text = extract_pdf_text_cached(file_path)?;
    let text_lower = text.to_lowercase();
    
    // 检查是否包含任何关键词
    let mut found_keywords = Vec::new();
    for keyword in keywords {
        if text_lower.contains(&keyword.to_lowercase()) {
            found_keywords.push(keyword.clone());
        }
    }
    
    if !found_keywords.is_empty() {
        // 找到包含关键词的上下文
        let lines: Vec<&str> = text.lines().collect();
        let mut context_lines = Vec::new();
        
        for (i, line) in lines.iter().enumerate() {
            let line_lower = line.to_lowercase();
            if found_keywords.iter().any(|k| line_lower.contains(&k.to_lowercase())) {
                // 包含前后各2行作为上下文
                let start = if i >= 2 { i - 2 } else { 0 };
                let end = if i + 3 < lines.len() { i + 3 } else { lines.len() };
                
                for j in start..end {
                    if !context_lines.contains(&lines[j]) {
                        context_lines.push(lines[j]);
                    }
                }
                
                if context_lines.len() > 10 { // 限制上下文行数
                    break;
                }
            }
        }
        
        let context = context_lines.join("\n").trim().to_string();
        let context = if context.len() > 300 {
            format!("{}...", &context[..300])
        } else {
            context
        };

        Ok(Some(SearchResult {
            file_path: file_path.to_string(),
            matched_text: context,
            file_size,
        }))
    } else {
        Ok(None)
    }
}

#[tauri::command]
async fn search_pdfs(
    config: SearchConfig,
    app_handle: tauri::AppHandle,
) -> Result<Vec<SearchResult>, String> {
    let folder_path = config.folder_path.clone();
    let keyword = config.keyword.clone();
    
    // 先收集所有PDF文件
    let pdf_files: Vec<_> = WalkDir::new(&folder_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|entry| {
            let path = entry.path();
            path.is_file() && path.extension().map_or(false, |ext| ext == "pdf")
        })
        .map(|entry| entry.path().to_string_lossy().to_string())
        .collect();

    if pdf_files.is_empty() {
        return Ok(Vec::new());
    }

    // 使用Arc<Mutex<Vec<SearchResult>>>来收集结果
    let results = Arc::new(Mutex::new(Vec::new()));
    let total_files = pdf_files.len();
    let processed_count = Arc::new(Mutex::new(0));

    // 使用rayon进行并行处理
    pdf_files.par_iter().for_each(|file_path| {
        // 更新进度
        {
            let mut count = processed_count.lock().unwrap();
            *count += 1;
            let progress = SearchProgress {
                current: *count,
                total: total_files,
                current_file: file_path.clone(),
            };
            
            // 发送进度事件（可选）
            let _ = app_handle.emit("search-progress", &progress);
        }

        // 搜索PDF
        if let Ok(Some(result)) = search_in_pdf(file_path, &keyword) {
            results.lock().unwrap().push(result);
        }
    });

    // 提取最终结果
    let final_results = results.lock().unwrap().clone();
    
    Ok(final_results)
}

// 批量搜索命令，支持多关键词和进度报告
#[tauri::command]
async fn search_pdfs_advanced(
    config: SearchConfig,
    app_handle: tauri::AppHandle,
) -> Result<Vec<SearchResult>, String> {
    let folder_path = config.folder_path.clone();
    
    // 支持多个关键词，用空格或逗号分隔
    let keywords: Vec<String> = config.keyword
        .split(&[' ', ',', ';'][..])
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if keywords.is_empty() {
        return Err("请输入有效的关键词".to_string());
    }
    
    // 收集所有PDF文件
    let pdf_files: Vec<_> = WalkDir::new(&folder_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|entry| {
            let path = entry.path();
            path.is_file() && path.extension().map_or(false, |ext| ext == "pdf")
        })
        .map(|entry| entry.path().to_string_lossy().to_string())
        .collect();

    if pdf_files.is_empty() {
        return Ok(Vec::new());
    }

    // 按文件大小排序，小文件优先处理
    let mut sorted_files: Vec<_> = pdf_files.into_iter().map(|path| {
        let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        (path, size)
    }).collect();
    sorted_files.sort_by(|a, b| a.1.cmp(&b.1));

    let total_files = sorted_files.len();
    let results = Arc::new(Mutex::new(Vec::new()));
    let processed_count = Arc::new(Mutex::new(0));

    // 并行搜索
    sorted_files.par_iter().for_each(|(file_path, _)| {
        // 更新进度
        {
            let mut count = processed_count.lock().unwrap();
            *count += 1;
            let progress = SearchProgress {
                current: *count,
                total: total_files,
                current_file: file_path.clone(),
            };
            
            let _ = app_handle.emit("search-progress", &progress);
        }

        // 执行搜索
        if let Ok(Some(result)) = quick_search_in_pdf(file_path, &keywords) {
            results.lock().unwrap().push(result);
        }
    });

    let final_results = results.lock().unwrap().clone();
    Ok(final_results)
}

#[tauri::command]
async fn select_folder(app_handle: tauri::AppHandle) -> Result<String, String> {
    use tauri_plugin_dialog::DialogExt;
    use std::sync::{Arc, Mutex};
    use std::sync::mpsc;
    
    let (sender, receiver) = mpsc::channel();
    let sender = Arc::new(Mutex::new(Some(sender)));
    
    app_handle
        .dialog()
        .file()
        .pick_folder(move |result| {
            if let Some(sender) = sender.lock().unwrap().take() {
                let _ = sender.send(result);
            }
        });
    
    match receiver.recv() {
        Ok(Some(path)) => Ok(path.to_string()),
        Ok(None) => Err("No folder selected".to_string()),
        Err(_) => Err("Failed to receive dialog result".to_string()),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![search_pdfs, select_folder, search_pdfs_advanced])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
