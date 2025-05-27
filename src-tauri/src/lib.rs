use anyhow::Result;
use base64::{engine::general_purpose, Engine as _};
use pdf_extract::extract_text_by_pages;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::RwLock;
use std::sync::{Arc, Mutex};
use std::vec::Vec;
use tauri::Emitter;
use walkdir::WalkDir;

#[derive(Debug, Serialize, Clone)]
struct PageInfoMatched {
    page_number: u32,
    matched_text: String,
}

#[derive(Debug, Serialize, Clone)]
struct SearchResult {
    file_path: String,
    file_size: u64,
    page_info: Vec<PageInfoMatched>,
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

#[derive(Debug, Serialize, Clone)]
struct PdfPageInfo {
    page_number: u32,
    content: String,
}

#[derive(Debug, Serialize)]
struct PdfViewerData {
    file_path: String,
    total_pages: u32,
    current_page: u32,
    page_content: String,
}

fn extract_pdf_text(file_path: &str) -> Result<Vec<String>> {
    let text = extract_text_by_pages(file_path)?;
    Ok(text)
}

// fn search_in_pdf(file_path: &str, keyword: &str) -> Option<SearchResult> {
//     let path = Path::new(file_path);
//     let file_size = path.metadata().ok()?.len();

//     let text = extract_pdf_text(file_path).ok()?;
//     let mut result = None;

//     for (page_num, page_text) in text.iter().enumerate() {
//         if page_text.is_empty() {
//             continue; // 跳过空白页
//         }
//         let lower_text = page_text.to_lowercase();
//         let keyword_lower = keyword.to_lowercase();
//         let char_indices = lower_text
//             .char_indices()
//             .map(|(i, _)| i)
//             .collect::<Vec<_>>();
//         let total_chars = char_indices.len();

//         if let Some((byte_idx, matched)) = lower_text.match_indices(&keyword_lower).next() {
//             let char_idx = char_indices.binary_search(&byte_idx).unwrap();

//             let start_idx = char_idx.saturating_sub(30); // 前30个字符
//             let end_idx = (char_idx + matched.chars().count() + 30).min(total_chars - 1); // 后30个字符

//             let start_byte = char_indices[start_idx];
//             let end_byte = char_indices[end_idx];
//             let matched_text = &page_text[start_byte..end_byte];

//             result
//                 .get_or_insert(SearchResult {
//                     file_path: file_path.to_string(),
//                     file_size,
//                     page_info: vec![],
//                 })
//                 .page_info
//                 .push(PageInfoMatched {
//                     page_number: (page_num + 1) as u32, // 页码从1开始
//                     matched_text: matched_text.to_string(),
//                 });
//         }
//     }

//     result
// }

// 简单的内存缓存
lazy_static::lazy_static! {
    static ref TEXT_CACHE: RwLock<HashMap<String, Vec<String>>> = RwLock::new(HashMap::new());
}

// 优化的文本提取函数，带缓存
fn extract_pdf_text_cached(file_path: &str) -> Result<Vec<String>> {
    // 检查缓存
    {
        let cache = TEXT_CACHE.read().unwrap();
        if let Some(cached_text) = cache.get(file_path) {
            return Ok(cached_text.clone());
        }
    }

    // 如果没有缓存，提取文本
    let text = extract_pdf_text(file_path)?;

    // 存入缓存（限制缓存大小）
    {
        let mut cache = TEXT_CACHE.write().unwrap();
        if cache.len() < 100 {
            // 限制缓存条目数
            cache.insert(file_path.to_string(), text.clone());
        }
    }

    Ok(text)
}

// 快速搜索函数，使用缓存提高性能
fn quick_search_in_pdf(file_path: &str, keyword: &str) -> Option<SearchResult> {
    let path = Path::new(file_path);
    let file_size = path.metadata().ok()?.len();

    // 使用缓存的文本提取
    let text = extract_pdf_text_cached(file_path).ok()?;
    let mut result = None;

    for (page_num, page_text) in text.iter().enumerate() {
        if page_text.is_empty() {
            continue; // 跳过空白页
        }
        let lower_text = page_text.to_lowercase();
        let keyword_lower = keyword.to_lowercase();
        let char_indices = lower_text
            .char_indices()
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        let total_chars = char_indices.len();

        if let Some((byte_idx, matched)) = lower_text.match_indices(&keyword_lower).next() {
            let char_idx = char_indices.binary_search(&byte_idx).unwrap();

            let start_idx = char_idx.saturating_sub(30); // 前30个字符
            let end_idx = (char_idx + matched.chars().count() + 30).min(total_chars - 1); // 后30个字符

            let start_byte = char_indices[start_idx];
            let end_byte = char_indices[end_idx];
            let matched_text = &page_text[start_byte..end_byte];

            result
                .get_or_insert(SearchResult {
                    file_path: file_path.to_string(),
                    file_size,
                    page_info: vec![],
                })
                .page_info
                .push(PageInfoMatched {
                    page_number: (page_num + 1) as u32, // 页码从1开始
                    matched_text: matched_text.to_string(),
                });
        }
    }

    result
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
            path.is_file() && path.extension().is_some_and(|ext| ext == "pdf")
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
        if let Some(result) = quick_search_in_pdf(file_path, &keyword) {
            results.lock().unwrap().push(result);
        }
    });

    // 提取最终结果
    let mut final_results = results.lock().unwrap().clone();
    final_results.sort_by(|a, b| a.file_path.cmp(&b.file_path));
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
    let keywords: Vec<String> = config
        .keyword
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
            path.is_file() && path.extension().is_some_and(|ext| ext == "pdf")
        })
        .map(|entry| entry.path().to_string_lossy().to_string())
        .collect();

    if pdf_files.is_empty() {
        return Ok(Vec::new());
    }

    // 按文件大小排序，小文件优先处理
    let mut sorted_files: Vec<_> = pdf_files
        .into_iter()
        .map(|path| {
            let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            (path, size)
        })
        .collect();
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
            };            let _ = app_handle.emit("search-progress", &progress);
        }

        // 执行搜索 - 对每个关键词分别搜索
        for keyword in &keywords {
            if let Some(result) = quick_search_in_pdf(file_path, keyword) {
                results.lock().unwrap().push(result);
                break; // 找到匹配就停止，避免重复结果
            }
        }
    });

    let mut final_results = results.lock().unwrap().clone();
    final_results.sort_by(|a, b| a.file_path.cmp(&b.file_path));
    Ok(final_results)
}

#[tauri::command]
async fn select_folder(app_handle: tauri::AppHandle) -> Result<String, String> {
    use std::sync::mpsc;
    use std::sync::{Arc, Mutex};
    use tauri_plugin_dialog::DialogExt;

    let (sender, receiver) = mpsc::channel();
    let sender = Arc::new(Mutex::new(Some(sender)));

    app_handle.dialog().file().pick_folder(move |result| {
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

// 获取PDF指定页面的内容
fn get_pdf_page_content(file_path: &str, page_number: u32) -> Result<String> {
    use lopdf::Document;

    let doc = Document::load(file_path)?;
    let content = doc.extract_text(&[page_number])?;
    Ok(content)
}

// 获取PDF总页数
fn get_pdf_page_count(file_path: &str) -> Result<u32> {
    use lopdf::Document;

    let doc = Document::load(file_path)?;
    let pages = doc.get_pages();
    Ok(pages.len() as u32)
}

// 打开PDF到指定页码的命令
#[tauri::command]
async fn open_pdf_at_page(file_path: String, page_number: Option<u32>) -> Result<(), String> {
    // 获取应用句柄 - 这需要从调用上下文中传递
    // 对于现在，我们使用标准的文件关联打开
    match std::process::Command::new("cmd")
        .args(["/c", "start", "", &file_path])
        .spawn()
    {
        Ok(_) => {
            if let Some(page) = page_number {
                eprintln!("PDF已打开，建议导航到第 {} 页", page);
            }
            Ok(())
        }
        Err(e) => Err(format!("无法打开PDF文件: {}", e)),
    }
}

// 获取PDF文件的base64编码用于前端显示
#[tauri::command]
async fn get_pdf_base64(file_path: String) -> Result<String, String> {
    use std::fs;

    println!("尝试读取PDF文件: {}", file_path);

    // 检查文件是否存在
    if !std::path::Path::new(&file_path).exists() {
        return Err(format!("文件不存在: {}", file_path));
    }

    // 检查文件大小
    let metadata = fs::metadata(&file_path).map_err(|e| format!("无法获取文件信息: {}", e))?;

    let file_size = metadata.len();
    println!("PDF文件大小: {} bytes", file_size);

    if file_size > 50 * 1024 * 1024 {
        // 限制50MB
        return Err("PDF文件过大 (超过50MB)".to_string());
    }

    let pdf_data = fs::read(&file_path).map_err(|e| format!("无法读取PDF文件: {}", e))?;

    println!("成功读取PDF文件，数据长度: {} bytes", pdf_data.len());

    let base64_data = general_purpose::STANDARD.encode(&pdf_data);
    println!("Base64编码完成，长度: {} chars", base64_data.len());

    Ok(base64_data)
}

// 获取PDF查看器数据的命令
#[tauri::command]
async fn get_pdf_viewer_data(
    file_path: String,
    page_number: Option<u32>,
) -> Result<PdfViewerData, String> {
    let total_pages =
        get_pdf_page_count(&file_path).map_err(|e| format!("无法获取PDF页数: {}", e))?;

    let current_page = page_number.unwrap_or(1).min(total_pages).max(1);

    let page_content = get_pdf_page_content(&file_path, current_page)
        .map_err(|e| format!("无法获取页面内容: {}", e))?;

    Ok(PdfViewerData {
        file_path,
        total_pages,
        current_page,
        page_content,
    })
}

// 搜索PDF中的文本并返回所有匹配页面
#[tauri::command]
async fn search_in_pdf_pages(
    file_path: String,
    search_text: String,
) -> Result<Vec<PdfPageInfo>, String> {
    let total_pages =
        get_pdf_page_count(&file_path).map_err(|e| format!("无法获取PDF页数: {}", e))?;

    let mut matching_pages = Vec::new();
    let search_lower = search_text.to_lowercase();

    for page_num in 1..=total_pages {
        if let Ok(content) = get_pdf_page_content(&file_path, page_num) {
            if content.to_lowercase().contains(&search_lower) {
                matching_pages.push(PdfPageInfo {
                    page_number: page_num,
                    content: content.lines().take(10).collect::<Vec<_>>().join("\n"), // 只取前10行作为预览
                });
            }
        }
    }

    Ok(matching_pages)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            search_pdfs,
            select_folder,
            search_pdfs_advanced,
            open_pdf_at_page,
            get_pdf_viewer_data,
            search_in_pdf_pages,
            get_pdf_base64
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
