<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { open } from "@tauri-apps/plugin-shell";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";

  let folderPath = $state("");
  let keyword = $state("");
  let results = $state<Array<{file_path: string, matched_text: string, file_size: number}>>([]);
  let searching = $state(false);
  let error = $state("");
  let useAdvancedSearch = $state(false);
  let searchProgress = $state<{current: number, total: number, current_file: string} | null>(null);

  onMount(() => {
    // 监听搜索进度事件
    const unlisten = listen<{current: number, total: number, current_file: string}>(
      'search-progress', 
      (event) => {
        searchProgress = event.payload;
      }
    );

    return () => {
      unlisten.then(fn => fn());
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
      const searchCommand = useAdvancedSearch ? "search_pdfs_advanced" : "search_pdfs";
      results = await invoke(searchCommand, {
        config: {
          folder_path: folderPath,
          keyword: keyword
        }
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
      await open(path);
    } catch (e) {
      error = e as string;
    }
  }

  function formatFileSize(bytes: number): string {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  }
</script>

<main class="container">
  <h1>PDF搜索工具</h1>

  <div class="search-form">
    <div class="folder-select">
      <button onclick={selectFolder}>选择文件夹</button>
      {#if folderPath}
        <span class="folder-path">{folderPath}</span>
      {/if}
    </div>

    <form class="search-input" onsubmit={searchPDFs}>
      <div class="search-options">
        <label>
          <input type="checkbox" bind:checked={useAdvancedSearch} />
          高级搜索 (支持多关键词，逗号分隔)
        </label>
      </div>
      <div class="input-group">
        <input
          placeholder={useAdvancedSearch ? "输入搜索关键词，用逗号分隔..." : "输入搜索关键词..."}
          bind:value={keyword}
          disabled={searching}
        />
        <button type="submit" disabled={searching}>
          {searching ? '搜索中...' : '搜索'}
        </button>
      </div>
    </form>

    {#if searching && searchProgress}
      <div class="progress-container">
        <div class="progress-info">
          <span>正在搜索: {searchProgress.current} / {searchProgress.total}</span>
          <span class="current-file">{searchProgress.current_file.split('\\').pop()}</span>
        </div>
        <div class="progress-bar">
          <div 
            class="progress-fill" 
            style="width: {(searchProgress.current / searchProgress.total) * 100}%"
          ></div>
        </div>
      </div>
    {/if}
  </div>

  {#if error}
    <div class="error">{error}</div>
  {/if}

  {#if results.length > 0}
    <div class="results">
      <h2>搜索结果 ({results.length})</h2>
      {#each results as result}
        <div class="result-item">
          <div class="result-header">
            <span class="file-path">{result.file_path}</span>
            <div class="file-info">
              <span class="file-size">{formatFileSize(result.file_size)}</span>
              <button onclick={() => openPDF(result.file_path)}>打开</button>
            </div>
          </div>
          <div class="matched-text">{result.matched_text}</div>
        </div>
      {/each}
    </div>
  {:else if !searching && !error}
    <p class="no-results">暂无搜索结果</p>
  {/if}
</main>

<style>
  .container {
    margin: 0 auto;
    padding: 2rem;
    max-width: 1200px;
  }

  h1 {
    text-align: center;
    margin-bottom: 2rem;
    color: #333;
  }

  .search-form {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    margin-bottom: 2rem;
  }

  .folder-select {
    display: flex;
    gap: 1rem;
    align-items: center;
  }

  .folder-path {
    font-size: 0.9em;
    color: #666;
    word-break: break-all;
  }

  .search-input {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .search-options {
    margin-bottom: 0.5rem;
  }

  .search-options label {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.9rem;
    color: #666;
  }

  .input-group {
    display: flex;
    gap: 0.5rem;
  }

  .progress-container {
    margin-top: 1rem;
    padding: 1rem;
    background: #f8f9fa;
    border-radius: 8px;
    border: 1px solid #e9ecef;
  }

  .progress-info {
    display: flex;
    justify-content: space-between;
    margin-bottom: 0.5rem;
    font-size: 0.9rem;
    color: #666;
  }

  .current-file {
    font-weight: 500;
    color: #495057;
    max-width: 300px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .progress-bar {
    width: 100%;
    height: 8px;
    background-color: #e9ecef;
    border-radius: 4px;
    overflow: hidden;
  }

  .progress-fill {
    height: 100%;
    background: linear-gradient(90deg, #007bff, #0056b3);
    transition: width 0.3s ease;
  }

  input {
    flex: 1;
    padding: 0.5rem 1rem;
    border: 1px solid #ddd;
    border-radius: 4px;
    font-size: 1rem;
  }

  button {
    padding: 0.5rem 1rem;
    border: none;
    border-radius: 4px;
    background-color: #0366d6;
    color: white;
    cursor: pointer;
    font-size: 1rem;
    min-width: 100px;
  }

  button:disabled {
    background-color: #ccc;
    cursor: not-allowed;
  }

  button:hover:not(:disabled) {
    background-color: #0256b5;
  }

  .error {
    background-color: #f8d7da;
    color: #721c24;
    padding: 1rem;
    border-radius: 4px;
    margin-bottom: 1rem;
  }

  .results {
    margin-top: 2rem;
  }

  .result-item {
    background-color: #f9f9f9;
    padding: 1rem;
    margin-bottom: 1rem;
    border-radius: 4px;
    border: 1px solid #e1e4e8;
  }

  .result-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 0.5rem;
  }

  .file-info {
    display: flex;
    align-items: center;
    gap: 1rem;
  }

  .file-size {
    font-size: 0.8rem;
    color: #666;
    background: #f0f0f0;
    padding: 0.2rem 0.5rem;
    border-radius: 3px;
  }

  .file-path {
    font-weight: bold;
    color: #0366d6;
    word-break: break-all;
    flex: 1;
  }

  .matched-text {
    background-color: #fff;
    padding: 0.5rem;
    border-left: 3px solid #0366d6;
    font-family: 'Courier New', monospace;
    font-size: 0.9rem;
    line-height: 1.4;
    white-space: pre-wrap;
  }

  .no-results {
    text-align: center;
    color: #666;
    font-style: italic;
    margin-top: 2rem;
  }
</style>
