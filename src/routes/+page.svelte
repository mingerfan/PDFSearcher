<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { open } from "@tauri-apps/plugin-shell";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import PDFViewer from "$lib/PDFViewer.svelte";

  let folderPath = $state("");
  let keyword = $state("");
  type PageInfoMatched = {
    page_number: number,
    matched_text: string,
  };
  let results = $state<
    Array<{ file_path: string; file_size: number; page_info: PageInfoMatched[] }>
  >([]);
  let searching = $state(false);
  let error = $state("");
  let useAdvancedSearch = $state(false);
  let searchProgress = $state<{
    current: number;
    total: number;
    current_file: string;
  } | null>(null);
  let pageToast = $state<{ message: string; visible: boolean }>({
    message: "",
    visible: false,
  });
  
  // PDF查看器状态
  let pdfViewer = $state<{
    visible: boolean;
    filePath: string;
    initialPage: number;
  }>({
    visible: false,
    filePath: "",
    initialPage: 1,
  });

  onMount(() => {
    // 监听搜索进度事件
    const unlisten = listen<{
      current: number;
      total: number;
      current_file: string;
    }>("search-progress", (event) => {
      searchProgress = event.payload;
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  });

  async function selectFolder() {
    try {
      folderPath = await invoke("select_folder");
      error = "";
    } catch (e) {
      error = e as string;
    }
  }

  async function searchPDFs(event: Event) {
    event.preventDefault();
    if (!folderPath) {
      error = "请先选择文件夹";
      return;
    }
    if (!keyword) {
      error = "请输入搜索关键词";
      return;
    }

    error = "";
    searching = true;
    searchProgress = null;
    results = [];

    try {
      const searchCommand = useAdvancedSearch
        ? "search_pdfs_advanced"
        : "search_pdfs";
      results = await invoke(searchCommand, {
        config: {
          folder_path: folderPath,
          keyword: keyword,
        },
      });
    } catch (e) {
      error = e as string;
    } finally {
      searching = false;
      searchProgress = null;
    }
  }

  async function openPDF(path: string) {
    try {
      // 使用opener插件而不是shell
      await invoke("open_pdf_at_page", {
        filePath: path,
        pageNumber: null,
      });
    } catch (e) {
      console.error("打开PDF失败:", e);
      error = `打开PDF失败: ${e}`;
    }
  }

  async function openPDFAtPage(path: string, page?: number) {
    try {
      await invoke("open_pdf_at_page", {
        filePath: path,
        pageNumber: page,
      });
      
      // 显示页码提示
      if (page) {
        pageToast = {
          message: `PDF已打开，建议导航到第 ${page} 页`,
          visible: true,
        };
        
        // 3秒后隐藏提示
        setTimeout(() => {
          pageToast = { ...pageToast, visible: false };
        }, 3000);
      }
    } catch (e) {
      error = e as string;
    }
  }

  // 在应用内打开PDF查看器
  function openPDFViewer(path: string, page?: number) {
    pdfViewer = {
      visible: true,
      filePath: path,
      initialPage: page || 1,
    };
  }

  // 关闭PDF查看器
  function closePDFViewer() {
    pdfViewer = {
      visible: false,
      filePath: "",
      initialPage: 1,
    };
  }

  function formatFileSize(bytes: number): string {
    if (bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
  }
</script>

<main class="app-layout">
  <div class="container">
  <div class="search-form">
    <div class="form-header">
      <div class="form-section">
        <h3 class="section-title">
          <span class="icon">📁</span>
          选择搜索目录
        </h3>
        <div class="folder-select">
          <button onclick={selectFolder} class="folder-button">
            <span class="button-icon">📂</span>
            选择文件夹
          </button>
          {#if folderPath}
            <div class="folder-path">
              <span class="path-icon">📍</span>
              {folderPath}
            </div>
          {/if}
        </div>
      </div>

      <div class="form-section">
        <h3 class="section-title">
          <span class="icon">🔍</span>
          搜索设置
        </h3>
        <label class="advanced-toggle">
          <input type="checkbox" bind:checked={useAdvancedSearch} />
          <span class="toggle-slider"></span>
          <span class="toggle-text">
            <strong>高级搜索</strong>
            <small>支持多关键词，用逗号分隔</small>
          </span>
        </label>
      </div>
    </div>

    <form class="search-input" onsubmit={searchPDFs}>
      <div class="input-group">
        <div class="search-input-wrapper">
          <span class="search-icon">🔍</span>
          <input
            type="text"
            placeholder={useAdvancedSearch
              ? "输入搜索关键词，用逗号分隔..."
              : "输入搜索关键词..."}
            bind:value={keyword}
            disabled={searching}
          />
        </div>
        <button type="submit" disabled={searching} class="search-button">
          <span class="button-content">
            {#if searching}
              <span class="spinner"></span>
              搜索中...
            {:else}
              <span class="button-icon">🚀</span>
              开始搜索
            {/if}
          </span>
        </button>
      </div>
    </form>

    {#if searching && searchProgress}
      <div class="progress-container">
        <div class="progress-header">
          <h4 class="progress-title">
            <span class="progress-icon">⚡</span>
            搜索进行中
          </h4>
        </div>
        <div class="progress-info">
          <div class="progress-stats">
            <span class="stat-item">
              <span class="stat-number">{searchProgress.current}</span>
              <span class="stat-label">已处理</span>
            </span>
            <span class="stat-divider">/</span>
            <span class="stat-item">
              <span class="stat-number">{searchProgress.total}</span>
              <span class="stat-label">总计</span>
            </span>
          </div>
          <div class="current-file">
            <span class="file-icon">📄</span>
            {searchProgress.current_file.split("\\").pop()}
          </div>
        </div>
        <div class="progress-bar">
          <div
            class="progress-fill"
            style="width: {(searchProgress.current / searchProgress.total) * 100}%"
          ></div>
        </div>
        <div class="progress-percentage">
          {Math.round((searchProgress.current / searchProgress.total) * 100)}%
        </div>
      </div>
    {/if}
  </div>

  {#if error}
    <div class="error">{error}</div>
  {/if}

  {#if results.length > 0}
    <div class="results">
      <div class="results-header">
        <h2>
          <span class="results-icon">📋</span>
          搜索结果
          <span class="results-count">({results.length})</span>
        </h2>
      </div>
      
      <div class="results-grid">
        {#each results as result, index}
          <div class="result-item" style="animation-delay: {index * 0.1}s">
            <div class="result-header">
              <div class="file-info-main">
                <h4 class="file-path">
                  <span class="file-icon">📄</span>
                  {result.file_path.split("\\").pop()}
                </h4>
                <div class="file-meta">
                  <!-- {#if result.page_number}
                    <span class="page-number">
                      <span class="meta-icon">📖</span>
                      第 {result.page_number} 页
                    </span>
                  {/if} -->
                </div>
              </div>
              
              <div class="action-buttons">
                <span class="file-size">
                  <span class="meta-icon">💾</span>
                  {formatFileSize(result.file_size)}
                </span>
                
                <button 
                  onclick={() => openPDF(result.file_path)}
                  class="action-btn external-btn"
                  title="使用默认PDF阅读器打开"
                >
                  <span class="btn-icon">🔗</span>
                  <span class="btn-text">外部打开</span>
                </button>
                
                <button 
                  class="action-btn viewer-btn"
                  onclick={() => openPDFViewer(result.file_path, result.page_info?.at(0)?.page_number)}
                  title="在应用内查看PDF"
                >
                  <span class="btn-icon">👁️</span>
                  <span class="btn-text">内置查看</span>
                </button>
                
                <!-- {#if result.page_number}
                  <button 
                    class="action-btn page-btn"
                    onclick={() => openPDFAtPage(result.file_path, result.page_number)}
                    title="直接跳转到第 {result.page_number} 页"
                  >
                    <span class="btn-icon">🎯</span>
                    <span class="btn-text">跳转页面</span>
                  </button>
                {/if} -->
              </div>
            </div>
            
            <div class="matched-content">
              <div class="content-header">
                <span class="content-icon">💡</span>
                <span class="content-title">匹配内容</span>
              </div>
              <div class="matched-text">{result.page_info?.at(0)?.matched_text}</div>
            </div>
          </div>
        {/each}
      </div>
    </div>
  {:else if !searching && !error}
    <div class="no-results">
      <div class="no-results-content">
        <span class="no-results-icon">🔍</span>
        <h3>暂无搜索结果</h3>
        <p>请选择文件夹并输入关键词开始搜索</p>
      </div>
    </div>
  {/if}

  <!-- 页码提示Toast -->
  {#if pageToast.visible}
    <div class="toast">{pageToast.message}</div>
  {/if}

  <!-- PDF查看器 -->
  {#if pdfViewer.visible}
    <PDFViewer 
      filePath={pdfViewer.filePath}
      initialPage={pdfViewer.initialPage}
      onClose={closePDFViewer}
    />
  {/if}
  </div>
</main>

<style>
  :global(body) {
    margin: 0;
    padding: 0;
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
    background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
    min-height: 100vh;
  }

  .app-layout {
    min-height: 100vh;
    background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
    padding: 1rem 0;
  }

  .container {
    margin: 0 auto;
    padding: 0 2rem;
    max-width: 1200px;
  }

  .search-form {
    background: white;
    border-radius: 16px;
    padding: 1.5rem;
    margin-bottom: 1.5rem;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.1);
    backdrop-filter: blur(10px);
    border: 1px solid rgba(255, 255, 255, 0.2);
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
  }

  .form-header {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 2rem;
    align-items: start;
  }

  .form-section {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .section-title {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 1rem;
    font-weight: 600;
    color: #374151;
    margin: 0;
  }

  .icon {
    font-size: 0.875rem;
  }

  .folder-select {
    display: flex;
    gap: 1rem;
    align-items: center;
    flex-wrap: wrap;
  }

  .folder-button {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    background: linear-gradient(135deg, #f59e0b 0%, #d97706 100%);
    padding: 0.75rem 1.25rem;
    border-radius: 12px;
    color: white;
    font-weight: 600;
    box-shadow: 0 4px 12px rgba(245, 158, 11, 0.3);
    transition: all 0.3s ease;
  }

  .folder-button:hover:not(:disabled) {
    background: linear-gradient(135deg, #d97706 0%, #b45309 100%);
    transform: translateY(-2px);
    box-shadow: 0 8px 25px rgba(245, 158, 11, 0.4);
  }

  .button-icon {
    font-size: 0.875rem;
  }

  .folder-path {
    font-size: 0.9em;
    color: #6b7280;
    word-break: break-all;
    background: linear-gradient(135deg, #f8fafc 0%, #f1f5f9 100%);
    padding: 0.75rem 1rem;
    border-radius: 12px;
    border: 1px solid #e2e8f0;
    flex: 1;
    min-width: 200px;
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .path-icon {
    color: #10b981;
    font-size: 0.875rem;
  }

  .search-input {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .advanced-toggle {
    display: flex;
    align-items: center;
    gap: 1rem;
    cursor: pointer;
    padding: 1rem;
    background: linear-gradient(135deg, #f8fafc 0%, #f1f5f9 100%);
    border-radius: 16px;
    border: 1px solid #e2e8f0;
    transition: all 0.3s ease;
  }

  .advanced-toggle:hover {
    background: linear-gradient(135deg, #f1f5f9 0%, #e2e8f0 100%);
    transform: translateY(-1px);
  }

  .advanced-toggle input[type="checkbox"] {
    display: none;
  }

  .toggle-slider {
    width: 50px;
    height: 26px;
    background: #d1d5db;
    border-radius: 13px;
    position: relative;
    transition: all 0.3s ease;
    cursor: pointer;
  }

  .toggle-slider::before {
    content: '';
    position: absolute;
    top: 2px;
    left: 2px;
    width: 22px;
    height: 22px;
    background: white;
    border-radius: 50%;
    transition: all 0.3s ease;
    box-shadow: 0 2px 4px rgba(0, 0, 0, 0.2);
  }

  .advanced-toggle input[type="checkbox"]:checked + .toggle-slider {
    background: linear-gradient(135deg, #6366f1 0%, #8b5cf6 100%);
  }

  .advanced-toggle input[type="checkbox"]:checked + .toggle-slider::before {
    transform: translateX(24px);
  }

  .toggle-text {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .toggle-text strong {
    color: #374151;
    font-size: 1rem;
  }

  .toggle-text small {
    color: #6b7280;
    font-size: 0.875rem;
  }

  .input-group {
    display: flex;
    gap: 1rem;
    position: relative;
  }

  .search-input-wrapper {
    flex: 1;
    position: relative;
    display: flex;
    align-items: center;
  }

  .search-icon {
    position: absolute;
    left: 1.25rem;
    color: #9ca3af;
    font-size: 1rem;
    z-index: 1;
  }

  .search-button {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    min-width: 140px;
  }

  .button-content {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .spinner {
    width: 16px;
    height: 16px;
    border: 2px solid rgba(255, 255, 255, 0.3);
    border-radius: 50%;
    border-top: 2px solid white;
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    0% { transform: rotate(0deg); }
    100% { transform: rotate(360deg); }
  }

  .progress-container {
    margin-top: 1.5rem;
    padding: 1.5rem;
    background: linear-gradient(135deg, rgba(255, 255, 255, 0.95) 0%, rgba(248, 250, 252, 0.95) 100%);
    border-radius: 16px;
    border: 1px solid rgba(255, 255, 255, 0.3);
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.1);
    backdrop-filter: blur(10px);
  }

  .progress-header {
    margin-bottom: 1.5rem;
  }

  .progress-title {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 1rem;
    font-weight: 700;
    color: #374151;
    margin: 0;
  }

  .progress-icon {
    font-size: 1rem;
    animation: pulse 2s infinite;
  }

  .progress-info {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1rem;
    flex-wrap: wrap;
    gap: 1rem;
  }

  .progress-stats {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .stat-item {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.25rem;
  }

  .stat-number {
    font-size: 1.5rem;
    font-weight: 700;
    color: #6366f1;
  }

  .stat-label {
    font-size: 0.75rem;
    color: #6b7280;
    font-weight: 500;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .stat-divider {
    font-size: 1.25rem;
    color: #d1d5db;
    font-weight: 300;
  }

  .current-file {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-weight: 600;
    color: #374151;
    max-width: 300px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    background: white;
    padding: 0.5rem 1rem;
    border-radius: 12px;
    border: 1px solid #e2e8f0;
    box-shadow: 0 2px 4px rgba(0, 0, 0, 0.05);
  }

  .file-icon {
    color: #f59e0b;
    font-size: 0.875rem;
  }

  .progress-bar {
    width: 100%;
    height: 16px;
    background: linear-gradient(135deg, #f1f5f9 0%, #e2e8f0 100%);
    border-radius: 8px;
    overflow: hidden;
    position: relative;
    box-shadow: inset 0 2px 4px rgba(0, 0, 0, 0.1);
  }

  .progress-fill {
    height: 100%;
    background: linear-gradient(90deg, #6366f1, #8b5cf6, #a855f7);
    transition: width 0.3s ease;
    position: relative;
    overflow: hidden;
    border-radius: 8px;
  }

  .progress-fill::after {
    content: '';
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.4), transparent);
    animation: shimmer 2s infinite;
  }

  .progress-percentage {
    text-align: center;
    margin-top: 0.75rem;
    font-size: 1.125rem;
    font-weight: 700;
    color: #6366f1;
  }

  @keyframes shimmer {
    0% { transform: translateX(-100%); }
    100% { transform: translateX(200%); }
  }

  input[type="text"] {
    flex: 1;
    padding: 1rem 1.5rem 1rem 3rem;
    border: 2px solid #e2e8f0;
    border-radius: 16px;
    font-size: 1rem;
    transition: all 0.3s ease;
    background: white;
    box-shadow: 0 2px 4px rgba(0, 0, 0, 0.05);
  }

  input[type="text"]:focus {
    outline: none;
    border-color: #6366f1;
    box-shadow: 0 0 0 3px rgba(99, 102, 241, 0.1);
    transform: translateY(-2px);
  }

  input[type="text"]:disabled {
    background: #f8fafc;
    cursor: not-allowed;
  }

  button {
    padding: 1rem 2rem;
    border: none;
    border-radius: 16px;
    background: linear-gradient(135deg, #6366f1 0%, #8b5cf6 100%);
    color: white;
    cursor: pointer;
    font-size: 1rem;
    font-weight: 600;
    min-width: 120px;
    transition: all 0.3s ease;
    box-shadow: 0 4px 12px rgba(99, 102, 241, 0.3);
    position: relative;
    overflow: hidden;
  }

  button::before {
    content: '';
    position: absolute;
    top: 0;
    left: -100%;
    width: 100%;
    height: 100%;
    background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.2), transparent);
    transition: left 0.5s ease;
  }

  button:hover::before {
    left: 100%;
  }

  button:disabled {
    background: #9ca3af;
    cursor: not-allowed;
    box-shadow: none;
    transform: none;
  }

  button:hover:not(:disabled) {
    transform: translateY(-3px);
    box-shadow: 0 8px 25px rgba(99, 102, 241, 0.4);
  }

  button:active:not(:disabled) {
    transform: translateY(-1px);
  }

  .error {
    background: linear-gradient(135deg, #fef2f2 0%, #fee2e2 100%);
    color: #dc2626;
    padding: 1.5rem;
    border-radius: 16px;
    margin-bottom: 1.5rem;
    border: 1px solid #fecaca;
    box-shadow: 0 4px 6px rgba(220, 38, 38, 0.1);
    font-weight: 500;
  }

  .results {
    margin-top: 2rem;
  }

  .results-header {
    margin-bottom: 1.5rem;
  }

  .results h2 {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    color: white;
    font-size: 1.5rem;
    font-weight: 700;
    margin: 0;
    text-shadow: 0 2px 4px rgba(0, 0, 0, 0.3);
  }

  .results-icon {
    font-size: 1.25rem;
  }

  .results-count {
    background: rgba(255, 255, 255, 0.2);
    padding: 0.5rem 1rem;
    border-radius: 12px;
    font-size: 1rem;
    font-weight: 600;
    backdrop-filter: blur(10px);
  }

  .results-grid {
    display: grid;
    gap: 1rem;
  }

  .result-item {
    background: rgba(255, 255, 255, 0.95);
    padding: 1.5rem;
    border-radius: 16px;
    border: 1px solid rgba(255, 255, 255, 0.3);
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.1);
    backdrop-filter: blur(10px);
    transition: all 0.3s ease;
    animation: slideInUp 0.5s ease-out;
    animation-fill-mode: both;
  }

  .result-item:hover {
    transform: translateY(-8px);
    box-shadow: 0 20px 40px rgba(0, 0, 0, 0.15);
  }

  .result-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    margin-bottom: 1rem;
    gap: 1rem;
  }

  .file-info-main {
    flex: 1;
  }

  .file-path {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 1.125rem;
    font-weight: 700;
    color: #374151;
    margin: 0 0 0.5rem 0;
    word-break: break-all;
  }

  .file-meta {
    display: flex;
    gap: 1rem;
    flex-wrap: wrap;
  }

  .action-buttons {
    display: flex;
    gap: 0.75rem;
    flex-wrap: wrap;
    flex-shrink: 0;
  }

  .action-btn {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.5rem 0.875rem;
    border-radius: 10px;
    font-size: 0.8rem;
    font-weight: 600;
    min-width: auto;
    transition: all 0.3s ease;
    white-space: nowrap;
  }

  .external-btn {
    background: linear-gradient(135deg, #6366f1 0%, #8b5cf6 100%);
    box-shadow: 0 4px 12px rgba(99, 102, 241, 0.3);
  }

  .external-btn:hover:not(:disabled) {
    background: linear-gradient(135deg, #5b21b6 0%, #7c3aed 100%);
    transform: translateY(-2px);
    box-shadow: 0 8px 25px rgba(99, 102, 241, 0.4);
  }

  .viewer-btn {
    background: linear-gradient(135deg, #06b6d4 0%, #0891b2 100%);
    box-shadow: 0 4px 12px rgba(6, 182, 212, 0.3);
  }

  .viewer-btn:hover:not(:disabled) {
    background: linear-gradient(135deg, #0891b2 0%, #0e7490 100%);
    transform: translateY(-2px);
    box-shadow: 0 8px 25px rgba(6, 182, 212, 0.4);
  }

  .page-btn {
    background: linear-gradient(135deg, #10b981 0%, #059669 100%);
    box-shadow: 0 4px 12px rgba(16, 185, 129, 0.3);
  }

  .page-btn:hover:not(:disabled) {
    background: linear-gradient(135deg, #059669 0%, #047857 100%);
    transform: translateY(-2px);
    box-shadow: 0 8px 25px rgba(16, 185, 129, 0.4);
  }

  .btn-icon {
    font-size: 0.75rem;
  }

  .btn-text {
    font-size: 0.8rem;
  }

  .page-number {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.875rem;
    color: #6366f1;
    background: linear-gradient(135deg, #eef2ff 0%, #e0e7ff 100%);
    padding: 0.5rem 1rem;
    border-radius: 12px;
    font-weight: 600;
    border: 1px solid #c7d2fe;
  }

  .file-size {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.875rem;
    color: #6b7280;
    background: linear-gradient(135deg, #f8fafc 0%, #f1f5f9 100%);
    padding: 0.5rem 1rem;
    border-radius: 12px;
    border: 1px solid #e2e8f0;
    font-weight: 500;
  }

  .meta-icon {
    font-size: 0.875rem;
  }

  .matched-content {
    border-top: 1px solid #e2e8f0;
    padding-top: 1rem;
  }

  .content-header {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-bottom: 0.75rem;
  }

  .content-icon {
    font-size: 0.875rem;
    color: #f59e0b;
  }

  .content-title {
    font-size: 1rem;
    font-weight: 600;
    color: #374151;
  }

  .matched-text {
    background: linear-gradient(135deg, #f8fafc 0%, #f1f5f9 100%);
    padding: 1.25rem;
    border-left: 4px solid #6366f1;
    border-radius: 12px;
    font-family: 'SF Mono', 'Monaco', 'Inconsolata', 'Roboto Mono', monospace;
    font-size: 0.9rem;
    line-height: 1.6;
    white-space: pre-wrap;
    box-shadow: inset 0 2px 4px rgba(0, 0, 0, 0.05);
    border: 1px solid #e2e8f0;
    color: #374151;
  }

  .no-results {
    text-align: center;
    margin-top: 4rem;
    background: rgba(255, 255, 255, 0.1);
    padding: 3rem 2rem;
    border-radius: 24px;
    backdrop-filter: blur(10px);
    border: 1px solid rgba(255, 255, 255, 0.2);
  }

  .no-results-content {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 1rem;
  }

  .no-results-icon {
    font-size: 4rem;
    opacity: 0.6;
  }

  .no-results h3 {
    color: white;
    font-size: 1.5rem;
    font-weight: 600;
    margin: 0;
    text-shadow: 0 2px 4px rgba(0, 0, 0, 0.3);
  }

  .no-results p {
    color: rgba(255, 255, 255, 0.8);
    font-size: 1.1rem;
    margin: 0;
    max-width: 400px;
  }

  .toast {
    position: fixed;
    top: 20px;
    right: 20px;
    background: linear-gradient(135deg, #10b981 0%, #059669 100%);
    color: white;
    padding: 1rem 1.5rem;
    border-radius: 16px;
    box-shadow: 0 10px 25px rgba(16, 185, 129, 0.4);
    z-index: 1000;
    animation: slideInRight 0.3s ease-out;
    max-width: 350px;
    font-size: 0.9rem;
    font-weight: 500;
    backdrop-filter: blur(10px);
    border: 1px solid rgba(255, 255, 255, 0.2);
  }

  @keyframes slideInRight {
    from {
      transform: translateX(100%);
      opacity: 0;
    }
    to {
      transform: translateX(0);
      opacity: 1;
    }
  }

  /* 响应式设计 */
  @media (max-width: 768px) {
    .container {
      padding: 0 1rem;
    }

    .form-header {
      grid-template-columns: 1fr;
      gap: 1rem;
    }

    .search-form {
      padding: 1.5rem;
      border-radius: 16px;
    }

    .input-group {
      flex-direction: column;
    }

    .result-header {
      flex-direction: column;
      align-items: flex-start;
    }
    .toast {
      top: 10px;
      right: 10px;
      left: 10px;
      max-width: none;
    }
  }

  /* 深色模式支持 */
  @media (prefers-color-scheme: dark) {
    .search-form {
      background: rgba(17, 24, 39, 0.8);
      color: white;
    }

    .folder-path {
      background: rgba(55, 65, 81, 0.6);
      color: #d1d5db;
      border-color: rgba(75, 85, 99, 0.6);
    }

    input[type="text"] {
      background: rgba(31, 41, 55, 0.6);
      border-color: rgba(75, 85, 99, 0.6);
      color: white;
    }

    input[type="text"]:focus {
      border-color: #6366f1;
    }

    .result-item {
      background: rgba(17, 24, 39, 0.8);
      color: white;
    }

    .file-path {
      color: #e5e7eb;
    }

    .matched-text {
      background: rgba(31, 41, 55, 0.6);
      color: #e5e7eb;
      border-left-color: #6366f1;
    }
  }

  /* 加载动画 */
  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.5; }
  }

  button:disabled {
    animation: pulse 2s infinite;
  }

  /* 悬浮效果 */
  .search-form {
    transition: all 0.3s ease;
  }

  .search-form:hover {
    transform: translateY(-2px);
    box-shadow: 0 25px 50px rgba(0, 0, 0, 0.15);
  }
</style>
