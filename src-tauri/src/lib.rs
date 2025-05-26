use anyhow::Result;
use pdf_extract::extract_text;
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

#[derive(Debug, Serialize)]
struct SearchResult {
    file_path: String,
    matched_text: String,
}

#[derive(Debug, Deserialize)]
struct SearchConfig {
    folder_path: String,
    keyword: String,
}

fn extract_pdf_text(file_path: &str) -> Result<String> {
    let text = extract_text(file_path)?;
    Ok(text)
}

fn search_in_pdf(file_path: &str, keyword: &str) -> Result<Option<SearchResult>> {
    let text = extract_pdf_text(file_path)?;
    
    if text.to_lowercase().contains(&keyword.to_lowercase()) {
        // 找到包含关键词的上下文
        let context = text
            .lines()
            .find(|line| line.to_lowercase().contains(&keyword.to_lowercase()))
            .unwrap_or("")
            .to_string();

        Ok(Some(SearchResult {
            file_path: file_path.to_string(),
            matched_text: context,
        }))
    } else {
        Ok(None)
    }
}

#[tauri::command]
async fn search_pdfs(config: SearchConfig) -> Result<Vec<SearchResult>, String> {
    let mut results = Vec::new();

    for entry in WalkDir::new(&config.folder_path)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.is_file() && path.extension().map_or(false, |ext| ext == "pdf") {
            if let Ok(Some(result)) = search_in_pdf(
                path.to_str().unwrap_or_default(),
                &config.keyword,
            ) {
                results.push(result);
            }
        }
    }

    Ok(results)
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
        .invoke_handler(tauri::generate_handler![search_pdfs, select_folder])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
