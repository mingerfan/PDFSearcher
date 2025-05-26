<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { open } from "@tauri-apps/plugin-shell";

  let folderPath = $state("");
  let keyword = $state("");
  let results = $state<Array<{file_path: string, matched_text: string}>>([]);
  let searching = $state(false);
  let error = $state("");

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
    try {
      results = await invoke("search_pdfs", {
        config: {
          folder_path: folderPath,
          keyword: keyword
        }
      });
    } catch (e) {
      error = e as string;
    } finally {
      searching = false;
    }
  }

  async function openPDF(path: string) {
    try {
      await open(path);
    } catch (e) {
      error = e as string;
    }
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
      <input
        placeholder="输入搜索关键词..."
        bind:value={keyword}
        disabled={searching}
      />
      <button type="submit" disabled={searching}>
        {searching ? '搜索中...' : '搜索'}
      </button>
    </form>
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
            <button onclick={() => openPDF(result.file_path)}>打开</button>
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
    gap: 0.5rem;
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
    color: #dc3545;
    padding: 1rem;
    margin: 1rem 0;
    background-color: #f8d7da;
    border-radius: 4px;
  }

  .results {
    margin-top: 2rem;
  }

  .result-item {
    padding: 1rem;
    border: 1px solid #ddd;
    border-radius: 4px;
    margin-bottom: 1rem;
  }

  .result-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 0.5rem;
  }

  .file-path {
    font-weight: bold;
    word-break: break-all;
  }

  .matched-text {
    font-size: 0.9em;
    color: #666;
    white-space: pre-wrap;
    word-break: break-all;
  }

  .no-results {
    text-align: center;
    color: #666;
  }

  @media (prefers-color-scheme: dark) {
    .folder-path {
      color: #aaa;
    }

    .matched-text {
      color: #aaa;
    }

    .result-item {
      border-color: #444;
    }

    input {
      background-color: #1a1a1a;
      color: #fff;
      border-color: #444;
    }

    .error {
      background-color: #481b1d;
      color: #f8d7da;
    }
  }
</style>
